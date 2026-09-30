"""Offline request-budget checks for the opt-in Provider matrix; no credentials."""
import json
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'examples'))
from live_provider_matrix import BudgetClient, DefaultHttpxClient, select_bindings


class BudgetTests(unittest.TestCase):
    def test_paused_provider_cannot_enter_a_live_selection(self):
        bindings = select_bindings(None)
        self.assertEqual(bindings[0][0], 'nvidia')
        self.assertNotIn('kimi', [row[0] for row in bindings])
        for selection in ('kimi', 'nvidia,kimi', '', 'unknown', 'nvidia,nvidia'):
            with self.assertRaises(RuntimeError):
                select_bindings(selection)
        self.assertEqual([row[0] for row in select_bindings('zhipu,nvidia')], ['zhipu', 'nvidia'])

    def test_budget_and_destination_fail_before_any_http_send(self):
        request = SimpleNamespace(url=SimpleNamespace(host='127.0.0.1'), method='POST',
                                  content=json.dumps({'max_output_tokens': 2048}).encode())
        with BudgetClient() as client, patch.object(DefaultHttpxClient, 'send', return_value=None) as send:
            for _ in range(60):
                client.send(request)
            with self.assertRaises(RuntimeError):
                client.send(request)
            self.assertEqual(send.call_count, 60)
        for host, method, content in (
            ('external.invalid', 'POST', request.content),
            ('127.0.0.1', 'GET', request.content),
            ('127.0.0.1', 'POST', b'{"max_output_tokens":2049}'),
            ('127.0.0.1', 'POST', json.dumps({'max_output_tokens': 2048, 'padding': 'x' * (256 * 1024)}).encode()),
        ):
            with BudgetClient() as client, patch.object(DefaultHttpxClient, 'send') as send:
                invalid = SimpleNamespace(url=SimpleNamespace(host=host), method=method, content=content)
                with self.assertRaises(RuntimeError):
                    client.send(invalid)
                send.assert_not_called()
                self.assertEqual(client.sent, 0)


if __name__ == '__main__':
    unittest.main()
