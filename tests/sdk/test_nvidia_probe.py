"""Synthetic send guards and closed error metadata; no credentials or sockets."""
import copy
from datetime import datetime, timezone
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'examples'))
from live_nvidia_probe import NvidiaClient, retry_after_seconds, DefaultHttpxClient, MODEL, failure_facts
from live_gateway_probe import UnexpectedChatFinish


class NvidiaGuardTests(unittest.TestCase):
    def test_truncation_is_distinct_from_missing_terminal_and_private_error_text(self):
        facts = failure_facts(UnexpectedChatFinish('length'), 'request_or_delivery')
        self.assertTrue(facts['protocol_consumed'])
        self.assertEqual(facts['failure_stage'], 'terminal_oracle')
        unknown = failure_facts(UnexpectedChatFinish('synthetic-private-text'), 'request_or_delivery')
        self.assertNotIn('protocol_consumed', unknown)
        self.assertNotIn('synthetic-private-text', str(unknown))
        self.assertNotIn('synthetic-private-text', str(failure_facts(RuntimeError('synthetic-private-text'), 'scenario_oracle')))

    def test_destination_budget_and_history_are_checked_before_send(self):
        origin = 'http://127.0.0.1:1234'
        history = [{'role':'assistant','content':None,'reasoning_content':'synthetic reasoning',
                    'tool_calls':[{'id':'call1','type':'function','function':{'name':'lookup','arguments':'{"key":"alpha"}'}}]},
                   {'role':'tool','tool_call_id':'call1','content':'{"value":17}'}]
        with NvidiaClient(origin) as client, patch.object(DefaultHttpxClient, 'send') as send:
            client.expected_history = copy.deepcopy(history)
            body = {'model':MODEL,'messages':history,'max_completion_tokens':2048,'stream':False}
            for change in ('origin','path','budget','alias','history'):
                invalid = copy.deepcopy(body)
                url = origin + '/v1/chat/completions'
                if change == 'origin':url = 'http://127.0.0.1:1235/v1/chat/completions'
                if change == 'path':url = origin + '/v1/responses'
                if change == 'budget':invalid['max_completion_tokens'] = 2049
                if change == 'alias':invalid['max_tokens'] = 9999
                if change == 'history':del invalid['messages'][0]['reasoning_content']
                with self.assertRaises(RuntimeError):
                    client.send(client.build_request('POST',url,json=invalid))
            send.assert_not_called()
            send.return_value.status_code = 429
            send.return_value.headers = {'retry-after':'17'}
            request = client.build_request('POST',origin+'/v1/chat/completions',json=body)
            for _ in range(16):client.send(request)
            self.assertTrue(client.history_verified)
            self.assertEqual(client.retry_after,17)
            self.assertEqual(client.last_status,429)
            with self.assertRaises(RuntimeError):client.send(request)
            self.assertEqual(send.call_count,16)

    def test_retry_after_is_bounded_numeric_metadata_not_a_raw_header_dump(self):
        now = datetime(2026,9,29,12,0,0,tzinfo=timezone.utc)
        self.assertEqual(retry_after_seconds('7',now),7)
        self.assertEqual(retry_after_seconds('Tue, 29 Sep 2026 12:00:20 GMT',now),20)
        for value in (None,'private data','-1','86401','7\r\nsecret','x'*129):
            self.assertIsNone(retry_after_seconds(value,now))


if __name__ == '__main__':unittest.main()
