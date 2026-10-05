"""Independent standard Speech request and binary-response OpenAPI checks."""
import unittest

from test_image_schema import DOCUMENT, validator


class SpeechSchemaTests(unittest.TestCase):
    def test_request_controls_presence_and_deferred_branches(self):
        check = validator("SpeechRequest")
        request = {"model": "public-speech", "input": "你好", "voice": "alloy"}
        check.validate(request)
        for encoding in ("mp3", "opus", "aac", "flac", "wav", "pcm"):
            check.validate({**request, "response_format": encoding, "stream_format": "audio",
                            "instructions": "", "speed": 0.25})
        check.validate({**request, "speed": 4, "input": "你" * 4096})
        for field in ("model", "input", "voice"):
            changed = dict(request)
            del changed[field]
            self.assertFalse(check.is_valid(changed))
        for field in ("input", "voice", "instructions", "speed", "response_format", "stream_format"):
            self.assertFalse(check.is_valid({**request, field: None}))
        for controls in (
            {"voice": {"id": "voice_fixture"}}, {"voice": "alloy\n"},
            {"input": ""}, {"input": "你" * 4097}, {"instructions": "a" * 4097},
            {"stream_format": "sse"}, {"speed": True}, {"speed": "1"},
            {"speed": 0.249}, {"speed": 4.001}, {"response_format": "pcm16"},
            {"stream": False}, {"provider": None}, {"_openbridge": {}},
        ):
            self.assertFalse(check.is_valid({**request, **controls}))

    def test_success_is_binary_not_a_private_json_envelope(self):
        success = DOCUMENT["paths"]["/v1/audio/speech"]["post"]["responses"]["200"]
        self.assertNotIn("application/json", success["content"])
        self.assertNotIn("text/event-stream", success["content"])
        for media in ("application/octet-stream", "audio/mpeg", "audio/wav"):
            self.assertEqual(success["content"][media]["schema"]["format"], "binary")


if __name__ == "__main__":
    unittest.main()
