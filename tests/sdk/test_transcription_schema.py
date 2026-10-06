"""Independent abstract multipart schema and standard transcription result checks."""
import unittest
from test_image_schema import DOCUMENT, validator


class TranscriptionSchemaTests(unittest.TestCase):
    def test_single_file_json_branch(self):
        check = validator("TranscriptionRequest")
        request = {"model": "public-asr", "file": "synthetic-file"}
        check.validate(request)
        check.validate({**request, "language": "zh", "response_format": "json", "stream": False})
        for value in (
            {"model": "public-asr"}, {**request, "file": ""},
            {**request, "file": ["one", "two"]}, {**request, "language": None},
            {**request, "response_format": "verbose_json"}, {**request, "stream": True},
            {**request, "prompt": ""}, {**request, "temperature": 0},
            {**request, "provider": {}}, {**request, "file_url": "https://example.invalid"},
        ):
            self.assertFalse(check.is_valid(value))
        post = DOCUMENT["paths"]["/v1/audio/transcriptions"]["post"]
        self.assertEqual(set(post["requestBody"]["content"]), {"multipart/form-data"})

    def test_result_has_text_and_only_reported_duration(self):
        check = validator("TranscriptionResponse")
        check.validate({"text": ""})
        check.validate({"text": "Hi.", "usage": {"type": "duration", "seconds": 1}})
        for value in (
            {}, {"text": None}, {"text": "Hi.", "usage": None},
            {"text": "Hi.", "usage": {"type": "duration", "seconds": -1}},
            {"text": "Hi.", "usage": {"seconds": 1}},
            {"text": "Hi.", "language": "invented"},
            {"text": "Hi.", "_openbridge": {}},
        ):
            self.assertFalse(check.is_valid(value))


if __name__ == "__main__":
    unittest.main()
