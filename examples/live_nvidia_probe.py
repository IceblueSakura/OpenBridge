"""Opt-in NVIDIA-only boundary gate; no retries or concurrency.

OPENBRIDGE_NVIDIA_PROBE=1 via the pinned tests/sdk uv environment. The full
matrix is 14 requests; the hard send ceiling is 16, at most 2048 tokens/request.
OPENBRIDGE_NVIDIA_CASE (json,history,length,cancel) and DELIVERY (json,sse)
only narrow it. Optional OPENBRIDGE_NVIDIA_EFFORT=none requests an explicit
standard control for a bounded contrast, never a silent default change. This does not probe Kimi or select its credentials. Only synthetic
inputs and closed diagnostic metadata; no body/header/credential logs.
"""
import copy
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import subprocess
import time
import tomllib

from openai import OpenAI, DefaultHttpxClient, __version__
from live_gateway_probe import chat_result, UnexpectedChatFinish
from live_provider_matrix import TOOL

ROOT = Path(__file__).resolve().parents[1]
MODEL = 'nemotron-3-super'


def retry_after_seconds(value, now=None):
    """Normalize only bounded delay/date values; never return header text."""
    if not isinstance(value, str) or len(value) > 128 or '\r' in value or '\n' in value:
        return None
    if re.fullmatch(r'[0-9]{1,5}', value):
        delay = int(value)
    else:
        try:
            date = parsedate_to_datetime(value)
            if date.tzinfo is None:
                return None
            delay = max(0, int((date - (now or datetime.now(timezone.utc))).total_seconds()))
        except (ValueError, TypeError, OverflowError):
            return None
    return delay if delay <= 86400 else None


def failure_facts(error, stage):
    """A valid non-success terminal is an oracle failure, not broken transport."""
    facts = {'failure':type(error).__name__, 'failure_stage':stage}
    if isinstance(error, UnexpectedChatFinish):
        facts['observed_terminal'] = error.finish
        if error.finish in ('stop', 'tool_calls', 'length', 'content_filter'):
            facts.update(protocol_consumed=True, failure_stage='terminal_oracle')
    return facts


class NvidiaClient(DefaultHttpxClient):
    """Bind every actual SDK send to one owned listener and immutable history."""
    def __init__(self, origin):
        super().__init__(trust_env=False, follow_redirects=False)
        self.destination = origin + '/v1/chat/completions'
        self.sent = 0
        self.expected_history = None
        self.history_verified = False
        self.last_status = None
        self.retry_after = None

    def send(self, request, **kwargs):
        self.history_verified = False
        self.last_status = self.retry_after = None
        if (str(request.url) != self.destination or request.method != 'POST'
                or self.sent >= 16 or len(request.content) > 256 * 1024):
            raise RuntimeError('request destination or budget violation')
        body = json.loads(request.content)
        if (body.get('model') != MODEL or type(body.get('stream')) is not bool
                or type(body.get('max_completion_tokens')) is not int
                or body['max_completion_tokens'] not in (8, 2048)
                or 'max_tokens' in body or 'max_output_tokens' in body
                or self.expected_history is None or body.get('messages') != self.expected_history):
            raise RuntimeError('request budget or serialized history changed')
        self.history_verified = True
        self.sent += 1
        response = super().send(request, **kwargs)
        self.last_status = response.status_code
        self.retry_after = retry_after_seconds(response.headers.get('retry-after'))
        return response


def run():
    if os.environ.get('OPENBRIDGE_NVIDIA_PROBE') != '1' or __version__ != '3.19.0':
        raise RuntimeError('explicit gate and pinned SDK required')
    case_filter = os.environ.get('OPENBRIDGE_NVIDIA_CASE')
    delivery_filter = os.environ.get('OPENBRIDGE_NVIDIA_DELIVERY')
    effort = os.environ.get('OPENBRIDGE_NVIDIA_EFFORT')
    if effort not in (None, 'none'):
        raise RuntimeError('unselected effort comparison')
    if case_filter not in (None, 'json', 'history', 'length', 'cancel') or delivery_filter not in (None, 'json', 'sse'):
        raise RuntimeError('unknown filter')
    if case_filter == 'cancel' and delivery_filter == 'json':
        raise RuntimeError('empty matrix')
    pools = tomllib.loads((ROOT / 'config/upstream-credentials.toml').read_text())['credential_pools']
    selected = [pool for pool in pools if pool['id'] == 'nvidia-primary']
    if len(selected) != 1:
        raise RuntimeError('missing unique NVIDIA binding')
    key = secrets.token_urlsafe(32)
    env = {'OPENBRIDGE_BIND':'127.0.0.1:0','OPENBRIDGE_CLIENT_KEY':key,
           'OPENBRIDGE_NVIDIA_API_KEY':selected[0]['api_keys'][0]}
    for name in ('OPENBRIDGE_PROXY','https_proxy','HTTPS_PROXY','http_proxy','HTTP_PROXY','all_proxy','ALL_PROXY'):
        if os.environ.get(name):
            env['OPENBRIDGE_PROXY'] = os.environ[name]
            break
    reports = []
    directory = ROOT / 'testdata/runtime' / f'nvidia-boundaries-{time.time_ns()}'
    directory.mkdir(parents=True, exist_ok=False)
    server = subprocess.Popen([str(ROOT/'target/debug/openbridge')],cwd=ROOT,env=env,
                              stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(server.stdout, selectors.EVENT_READ)
            if not selector.select(timeout=15):
                raise RuntimeError('startup timeout')
            line = server.stdout.readline(256).strip()
        match = re.fullmatch(r'OpenBridge listening on (http://127\.0\.0\.1:\d+)', line)
        if not match:
            raise RuntimeError('missing readiness')
        transport = NvidiaClient(match[1])
        with OpenAI(base_url=match[1]+'/v1',api_key=key,max_retries=0,organization='',project='',
                    timeout=130,http_client=transport) as client:
            def call(case, streaming, history, *, terminal='stop', cap=2048, extra=None, cancel=False, oracle=None):
                report = {'case':case,'delivery':'sse' if streaming else 'json',
                          'max_tokens':cap,'reasoning_effort':effort or 'default','ok':False,'protocol_consumed':False}
                started = time.monotonic()
                stage = 'request_or_delivery'
                result = None
                try:
                    transport.expected_history = copy.deepcopy(history)
                    params = {'model':MODEL,'messages':history,'stream':streaming,'max_completion_tokens':cap, **(extra or {})}
                    if effort is not None:
                        params['reasoning_effort'] = effort
                    if streaming:
                        params['stream_options'] = {'include_usage':True,'include_obfuscation':False}
                    result = client.chat.completions.create(**params)
                    if not streaming:
                        message = result.choices[0].message.model_dump(mode='json',exclude_unset=True)
                        report.update(text_chars=len(message.get('content') or ''),
                                      reasoning_chars=len(message.get('reasoning_content') or ''))
                        tokens = getattr(result.usage,'completion_tokens',None)
                        if type(tokens) is int and 0 <= tokens <= 1000000:
                            report['reported_output_tokens'] = tokens
                    if cancel:
                        found = False
                        for index, chunk in enumerate(result):
                            assert index < 32
                            if not chunk.choices:
                                continue
                            choice = chunk.choices[0]
                            assert choice.finish_reason is None
                            data = choice.delta.model_dump(mode='json',exclude_unset=True)
                            if data.get('content') or data.get('reasoning_content'):
                                found = True
                                break
                        assert found
                        result.close()
                        output, text, calls = [], '', []
                        report['client_closed_before_terminal'] = True
                    else:
                        output, text, calls = chat_result(result,streaming,allowed_finishes=(terminal,))
                        report.update(protocol_consumed=True,terminal=terminal,text_chars=len(text),tool_calls=len(calls),
                                      reasoning_chars=sum(len(item.get('reasoning_content') or '') for item in output))
                    stage = 'scenario_oracle'
                    if oracle:
                        oracle(text,calls)
                    report['ok'] = True
                    return output, text, calls
                except Exception as error:
                    report.update(failure_facts(error,stage))
                    raise RuntimeError('scenario stopped') from None
                finally:
                    if streaming and result is not None:
                        result.close()
                    report.update(request=transport.sent,http=transport.last_status,
                                  retry_after_seconds=transport.retry_after,serialized_history_verified=transport.history_verified,
                                  elapsed_ms=round((time.monotonic()-started)*1000))
                    reports.append(report)
                    (directory/'calls.json').write_text(json.dumps(reports,indent=2))
                    print(json.dumps(report),flush=True)

            def check_json(text, calls):
                assert not calls and json.loads(text) == {'answer':7}

            def check_no_calls(text, calls):
                assert not calls

            def check_value(expected):
                def check(text, calls):
                    assert not calls and re.search(rf'(?<!\d){expected}(?!\d)',text)
                return check

            def check_call(expected):
                def check(text, calls):
                    assert len(calls) == 1
                    assert calls[0]['function']['name'] == 'lookup'
                    assert json.loads(calls[0]['function']['arguments']) == {'key':expected}
                return check

            for case in ('json','history','length'):
                if case_filter and case_filter != case:
                    continue
                for streaming in (False, True):
                    if delivery_filter and delivery_filter != ('sse' if streaming else 'json'):
                        continue
                    if case == 'json':
                        call(case,streaming,[{'role':'user','content':'Return only a JSON object with exactly one integer field answer equal to 7.'}],
                             extra={'response_format':{'type':'json_object'}},oracle=check_json)
                    elif case == 'history':
                        history = []
                        tools = {'tools':[{'type':'function','function':TOOL}],'tool_choice':'auto'}
                        for lookup_key, value in (('alpha',17),('beta',29)):
                            history.append({'role':'user','content':f'Use lookup for key {lookup_key}, then report its returned value in one short sentence.'})
                            output, _, calls = call(f'history-{lookup_key}-call',streaming,history,
                                                    terminal='tool_calls',extra=tools,oracle=check_call(lookup_key))
                            history.extend(output)
                            history.append({'role':'tool','tool_call_id':calls[0]['id'],'content':json.dumps({'value':value})})
                            output, _, _ = call(f'history-{lookup_key}-result',streaming,history,extra=tools,oracle=check_value(value))
                            history.extend(output)
                    else:
                        call(case,streaming,[{'role':'user','content':'Write the word alpha 200 times separated by spaces. Do not summarize.'}],
                             cap=8,terminal='length',oracle=check_no_calls)
            if case_filter in (None,'cancel') and delivery_filter != 'json':
                call('cancel',True,[{'role':'user','content':'Write the word alpha 200 times separated by spaces. Do not summarize.'}],cancel=True)
                def check_pong(text,calls):
                    assert text.strip() == 'pong' and not calls
                call('post-cancel',True,[{'role':'user','content':'Reply with exactly pong.'}],oracle=check_pong)
        return 0 if reports and all(report['ok'] for report in reports) else 1
    except Exception:
        print('NVIDIA batch stopped; private exception details suppressed. No retry.',flush=True)
        return 1
    finally:
        server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait()
        print(json.dumps({'reports':len(reports),'passed':sum(report['ok'] for report in reports),
                          'report':str(directory.relative_to(ROOT)/'calls.json')}),flush=True)


if __name__ == '__main__':
    try:
        raise SystemExit(run())
    except Exception:
        print('NVIDIA probe setup failed; private details suppressed.',flush=True)
        raise SystemExit(1)
