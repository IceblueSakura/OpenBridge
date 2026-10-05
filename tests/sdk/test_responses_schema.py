"""Independent summary-control expectations for the public Responses schema."""
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator


class ResponsesSchemaTests(unittest.TestCase):
    def test_summary_presence_and_invalid_carriers(self):
        document = json.loads(
            (Path(__file__).resolve().parents[2] / "docs/openapi.json").read_text()
        )
        schema = {
            "$ref": "#/components/schemas/ResponsesRequest",
            "components": document["components"],
        }
        Draft202012Validator.check_schema(schema)
        validator = Draft202012Validator(schema)
        base = {"model": "synthetic", "input": "hello"}
        validator.validate(base)
        for reasoning in (None, {}):
            validator.validate({**base, "reasoning": reasoning})
        for value in (None, "synthetic-session", {}):
            self.assertFalse(validator.is_valid({**base, "session_id": value}))
        for kind in ("function", "custom"):
            tool = {"type": kind, "name": "lookup"}
            validator.validate({**base, "tools": [tool]})
            for field in ("async", "defer_loading"):
                for value in (False, True):
                    validator.validate({**base, "tools": [{**tool, field: value}]})
                for value in (None, "false", 0):
                    self.assertFalse(validator.is_valid(
                        {**base, "tools": [{**tool, field: value}]}
                    ))
        for field in ("summary", "generate_summary"):
            for value in (None, "auto", "concise", "detailed"):
                validator.validate({**base, "reasoning": {field: value}})
            for value in (False, True, 0, "none", [], {}):
                with self.subTest(field=field, value=value):
                    self.assertFalse(
                        validator.is_valid({**base, "reasoning": {field: value}})
                    )


if __name__ == "__main__":
    unittest.main()
