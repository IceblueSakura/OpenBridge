"""Explicit paid, bounded Provider gate through the actual loopback binary.

OPENBRIDGE_PROVIDER_MATRIX=1 uv run --project tests/sdk --locked --offline python
examples/live_provider_matrix.py. Optional OPENBRIDGE_MATRIX_PROVIDERS is a comma
list of fixed provider names. NVIDIA first, Kimi paused. At most 60 requests/run,
2048 output tokens each;
no retries. Only synthetic text and function results are sent. Credentials and
raw body/headers are never logged or saved. Re-running requires fresh authorization.
"""
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import subprocess
import time
import tomllib

from openai import OpenAI, DefaultHttpxClient
from live_gateway_probe import chat_result, response_result

ROOT = Path(__file__).resolve().parents[1]
BINDINGS = (
    ('deepseek', 'deepseek-primary', 'OPENBRIDGE_DEEPSEEK_API_KEY', 'deepseek-flash', ('chat', 'responses')),
    ('xiaomi', 'mimo-primary', 'OPENBRIDGE_XIAOMI_API_KEY', 'mimo-v2.6-pro', ('chat', 'responses')),
    ('openrouter', 'openrouter-primary', 'OPENBRIDGE_OPENROUTER_API_KEY', 'gpt-6-luna', ('chat', 'responses')),
    ('longcat', 'longcat-primary', 'OPENBRIDGE_LONGCAT_API_KEY', 'longcat-2.5-preview', ('chat',)),
    ('nvidia', 'nvidia-primary', 'OPENBRIDGE_NVIDIA_API_KEY', 'nemotron-3-super', ('chat',)),
    ('bailian', 'bailian-primary', 'OPENBRIDGE_BAILIAN_API_KEY', 'qwen3.8-max', ('chat',)),
    ('kimi', 'kimi-primary', 'OPENBRIDGE_KIMI_API_KEY', 'kimi-k3', ('chat',)),
    ('zhipu', 'zhipu-primary', 'OPENBRIDGE_ZHIPU_API_KEY', 'glm-5.3', ('chat',)),
)
TOOL = {'name': 'lookup', 'description': 'Look up a synthetic value.',
        'parameters': {'type': 'object', 'properties': {'key': {'type': 'string'}},
                       'required': ['key'], 'additionalProperties': False}, 'strict': False}


def select_bindings(selection):
    """Reject paused/invalid choices before credentials or network are touched."""
    available = {row[0]: row for row in BINDINGS if row[0] != 'kimi'}
    names = selection.split(',') if selection is not None else ['nvidia'] + [name for name in available if name != 'nvidia']
    if not names or len(set(names)) != len(names) or any(name not in available for name in names):
        raise RuntimeError('unknown, duplicate or paused provider selection')
    return [available[name] for name in names]


class BudgetClient(DefaultHttpxClient):
    """Enforce the request ceiling at actual SDK send, not a loop counter."""
    def __init__(self):
        super().__init__(trust_env=False, follow_redirects=False)
        self.sent = 0

    def send(self, request, **kwargs):
        body = json.loads(request.content)
        if (request.url.host != '127.0.0.1' or request.method != 'POST' or self.sent >= 60
                or len(request.content) > 256 * 1024
                or body.get('max_completion_tokens', body.get('max_output_tokens')) != 2048):
            raise RuntimeError('request budget or target violation')
        self.sent += 1
        return super().send(request, **kwargs)


def run():
    if os.environ.get('OPENBRIDGE_PROVIDER_MATRIX') != '1':
        raise RuntimeError('explicit paid gate required')
    bindings = select_bindings(os.environ.get('OPENBRIDGE_MATRIX_PROVIDERS'))
    data = tomllib.loads((ROOT / 'config/upstream-credentials.toml').read_text())
    pools = {p['id']: p for p in data['credential_pools']}
    client_key = secrets.token_urlsafe(32)
    env = {'OPENBRIDGE_BIND': '127.0.0.1:0', 'OPENBRIDGE_CLIENT_KEY': client_key}
    for _, pool, variable, _, _ in bindings:
        env[variable] = pools[pool]['api_keys'][0]
    for name in ('OPENBRIDGE_PROXY', 'https_proxy', 'HTTPS_PROXY', 'http_proxy', 'HTTP_PROXY', 'all_proxy', 'ALL_PROXY'):
        if os.environ.get(name):
            env['OPENBRIDGE_PROXY'] = os.environ[name]
            break
    report_dir = ROOT / 'testdata/runtime' / f'provider-matrix-{time.time_ns()}'
    report_dir.mkdir(parents=True, exist_ok=False)
    delivery_filter = os.environ.get('OPENBRIDGE_MATRIX_DELIVERY')
    case_filter = os.environ.get('OPENBRIDGE_MATRIX_CASE')
    protocol_filter = os.environ.get('OPENBRIDGE_MATRIX_PROTOCOL')
    if delivery_filter not in (None, 'json', 'sse') or case_filter not in (None, 'text', 'tool') or protocol_filter not in (None, 'chat', 'responses'):
        raise RuntimeError('invalid matrix filter')
    reports = []
    server = subprocess.Popen([str(ROOT / 'target/debug/openbridge')], cwd=ROOT, env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(server.stdout, selectors.EVENT_READ)
            if not selector.select(timeout=15):
                raise RuntimeError('startup timeout')
            line = server.stdout.readline(256).strip()
        match = re.fullmatch(r'OpenBridge listening on (http://127\.0\.0\.1:\d+)', line)
        if not match:
            raise RuntimeError('missing readiness')
        transport = BudgetClient()
        with OpenAI(base_url=match[1] + '/v1', api_key=client_key, max_retries=0,
                    organization='', project='', timeout=130, http_client=transport) as client:
            for provider, _, _, model, protocols in bindings:
                blocked = False
                for protocol in protocols:
                    if protocol_filter and protocol != protocol_filter:
                        continue
                    if blocked:
                        break
                    for streaming in (False, True):
                        if delivery_filter and ('sse' if streaming else 'json') != delivery_filter:
                            continue
                        if blocked:
                            break
                        for case in ('text', 'tool'):
                            if case_filter and case != case_filter:
                                continue
                            if blocked:
                                break
                            history = [{'role': 'user', 'content': 'Reply with exactly pong.' if case == 'text'
                                        else 'Use lookup for key alpha, then report its value in one sentence.'}]
                            for turn in range(1, 2 if case == 'text' else 3):
                                report = {'provider': provider, 'model': model, 'protocol': protocol,
                                          'delivery': 'sse' if streaming else 'json', 'case': case, 'turn': turn, 'ok': False}
                                try:
                                    params = {'model': model, 'stream': streaming}
                                    if case == 'tool':
                                        params['tool_choice'] = ('auto' if provider == 'deepseek' or protocol == 'chat' and provider == 'nvidia'
                                                                 else 'required' if turn == 1 else 'none')
                                        params['tools'] = ([{'type': 'function', 'function': TOOL}] if protocol == 'chat'
                                                           else [{'type': 'function', **TOOL}])
                                    if protocol == 'chat':
                                        params.update(messages=history, max_completion_tokens=2048)
                                        if streaming:
                                            params['stream_options'] = {'include_usage': True, 'include_obfuscation': False}
                                        output, text, calls = chat_result(client.chat.completions.create(**params), streaming)
                                    else:
                                        params.update(input=history, max_output_tokens=2048)
                                        output, text, calls = response_result(client.responses.create(**params), streaming)
                                    if case == 'text':
                                        assert text.strip() == 'pong' and not calls
                                    elif turn == 1:
                                        assert len(calls) == 1
                                        call = calls[0]
                                        fn = call['function'] if protocol == 'chat' else call
                                        assert fn['name'] == 'lookup' and json.loads(fn['arguments']) == {'key': 'alpha'}
                                        history.extend(output)
                                        history.append({'role': 'tool', 'tool_call_id': call['id'], 'content': '{"value":42}'} if protocol == 'chat'
                                                       else {'type': 'function_call_output', 'call_id': call['call_id'], 'output': '{"value":42}'})
                                    else:
                                        assert '42' in text and not calls
                                    report['ok'] = True
                                except Exception as error:
                                    status = getattr(error, 'status_code', None)
                                    report['failure'] = type(error).__name__
                                    if isinstance(status, int):
                                        report['http'] = status
                                    # Do not repeat a failing provider in unchanged scenarios.
                                    blocked = True
                                reports.append(report)
                                print(json.dumps(report), flush=True)
                                (report_dir / 'calls.json').write_text(json.dumps(reports, indent=2))
                                if blocked:
                                    break
        print(json.dumps({'requests': transport.sent, 'passed': sum(r['ok'] for r in reports),
                          'report': str(report_dir.relative_to(ROOT) / 'calls.json')}), flush=True)
        return 0 if reports and all(r['ok'] for r in reports) else 1
    finally:
        server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()


if __name__ == '__main__':
    try:
        raise SystemExit(run())
    except Exception:
        print('Provider matrix failed; private exception details suppressed.', flush=True)
        raise SystemExit(1)
