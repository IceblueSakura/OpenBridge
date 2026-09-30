"""Independent typed collector fixtures; not produced by the runtime encoder."""

from pathlib import Path
from types import SimpleNamespace
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "examples"))
from probe_support.codecs import chat_result, opaque_records, UnexpectedChatFinish


class CollectorTests(unittest.TestCase):
    def test_length_requires_an_explicit_scenario(self):
        message = {"role": "assistant", "content": "partial"}
        static = SimpleNamespace(
            choices=[
                SimpleNamespace(
                    finish_reason="length",
                    message=SimpleNamespace(model_dump=lambda **_: message),
                )
            ]
        )
        chunk = SimpleNamespace(
            choices=[True],
            model_dump=lambda **_: {
                "choices": [
                    {
                        "index": 0,
                        "delta": {"content": "partial"},
                        "finish_reason": "length",
                    }
                ]
            },
        )
        for value, stream in ((static, False), ([chunk], True)):
            with self.assertRaises(UnexpectedChatFinish):
                chat_result(value, stream)
            _, text, calls = chat_result(value, stream, allowed_finishes=("length",))
            self.assertEqual((text, calls), ("partial", []))

    def test_scoped_opaque_values_are_not_confused_with_unrelated_fields(self):
        body = {
            "input": [
                {"type": "reasoning", "id": "rs1", "encrypted_content": "synthetic-a"}
            ],
            "messages": [
                {
                    "reasoning_details": [
                        {
                            "type": "reasoning.encrypted",
                            "id": "rs2",
                            "data": "synthetic-b",
                        }
                    ]
                }
            ],
            "data": "not-a-replay-token",
        }
        self.assertEqual(
            opaque_records(body), [("rs1", "synthetic-a"), ("rs2", "synthetic-b")]
        )


if __name__ == "__main__":
    unittest.main()
