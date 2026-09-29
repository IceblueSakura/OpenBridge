"""Offline tests for the live probe's memory-only replay guard; no credentials or sockets."""
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("live_gateway_probe", ROOT / "examples/live_gateway_probe.py")
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class ReplayGuardTest(unittest.TestCase):
    def test_both_carriers_extract_identity_and_token_without_unrelated_data(self):
        body = {"input": [{"type": "reasoning", "id": "rs1", "encrypted_content": "synthetic-a"}],
                "messages": [{"reasoning_details": [{"type": "reasoning.encrypted", "id": "rs2", "data": "synthetic-b"}]}],
                "data": "not-a-replay-token"}
        self.assertEqual(probe.opaque_records(body), [("rs1", "synthetic-a"), ("rs2", "synthetic-b")])

    def test_missing_or_rebound_replay_cannot_reach_send(self):
        with probe.ReplayCheckingClient(1) as client:
            client.expected = [("rs", "synthetic-token")]
            for item in ({}, {"type": "reasoning", "id": "different", "encrypted_content": "synthetic-token"}):
                request = client.build_request("POST", "http://127.0.0.1:1/v1/responses",
                                               json={"model": probe.MODEL, "input": [item]})
                with patch.object(probe.DefaultHttpxClient, "send") as send:
                    with self.assertRaises(RuntimeError):
                        client.send(request)
                    send.assert_not_called()
            self.assertEqual(client.sent, 0)

    def test_matching_serialized_body_is_sent_unchanged_and_count_is_bounded(self):
        with probe.ReplayCheckingClient(1) as client:
            client.expected = [("rs", "synthetic-token")]
            request = client.build_request("POST", "http://127.0.0.1:1/v1/responses",
                                           json={"model": probe.MODEL, "input": [{"type": "reasoning", "id": "rs", "encrypted_content": "synthetic-token"}]})
            with patch.object(probe.DefaultHttpxClient, "send", return_value="synthetic-response") as send:
                self.assertEqual(client.send(request), "synthetic-response")
                send.assert_called_once_with(request)
                self.assertTrue(client.verified)
                with self.assertRaises(RuntimeError):
                    client.send(request)
                self.assertEqual(send.call_count, 1)


if __name__ == "__main__":
    unittest.main()
