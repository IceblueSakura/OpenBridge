"""Independent Draft 2020-12 validation of the published Images schemas."""
import copy
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator

DOCUMENT = json.loads((Path(__file__).resolve().parents[2] / "docs/openapi.json").read_text())
SCHEMAS = DOCUMENT["components"]["schemas"]


def validator(name):
    return Draft202012Validator({
        "$ref": f"#/components/schemas/{name}",
        "components": {"schemas": SCHEMAS},
    })


class ImageSchemaTests(unittest.TestCase):
    def test_all_component_schemas_and_local_references_are_valid(self):
        for schema in SCHEMAS.values():
            Draft202012Validator.check_schema(schema)
        def visit(value):
            if isinstance(value, dict):
                if "$ref" in value:
                    self.assertTrue(value["$ref"].startswith("#/"))
                    target = DOCUMENT
                    for part in value["$ref"][2:].split("/"):
                        target = target[part.replace("~1", "/").replace("~0", "~")]
                    self.assertIsInstance(target, dict)
                for child in value.values():
                    visit(child)
            elif isinstance(value, list):
                for child in value:
                    visit(child)
        visit(DOCUMENT)

    def test_request_counts_presence_and_control_constraints(self):
        check = validator("ImageGenerationRequest")
        request = {"model": "public-image", "prompt": "synthetic"}
        check.validate(request)
        for n in (None, 1, 2, 10):
            check.validate({**request, "n": n})
        for n in (0, -1, 11, 1.5, True, "2", [], {}):
            self.assertFalse(check.is_valid({**request, "n": n}))
        for controls in (
            {"stream": True}, {"provider": {}}, {"_openbridge": {}},
            {"output_compression": 1}, {"output_format": "png", "output_compression": 0},
            {"output_format": "jpeg", "background": "transparent"},
            {"size": "0x1"}, {"size": "65537x1"}, {"user": None},
        ):
            self.assertFalse(check.is_valid({**request, **controls}))
        check.validate({**request, "n": 2, "output_format": "webp",
                        "background": "transparent", "output_compression": 80})

    def test_response_collection_shape_and_shared_reports(self):
        check = validator("ImageGenerationResponse")
        response = {"created": 7, "data": [{"b64_json": "AQID"}, {"b64_json": "BAUG"}],
                    "output_format": "png", "size": "2x3", "background": "opaque",
                    "quality": "high"}
        check.validate(response)
        for n in (1, 10):
            check.validate({**response, "data": [{"b64_json": "AQID"}] * n})
        for data in ([], None, [{"b64_json": "AQID"}] * 11,
                     [{"url": "https://example.invalid"}],
                     [{"b64_json": "AQID"}, {"b64_json": ""}],
                     [{"b64_json": "AQID", "media_type": "image/png"}]):
            self.assertFalse(check.is_valid({**response, "data": data}))
        changed = copy.deepcopy(response)
        changed.update(output_format="jpeg", background="transparent")
        self.assertFalse(check.is_valid(changed))
        for field in ("output_format", "size", "background", "quality", "usage"):
            check.validate({**response, field: None})
        # Base64 syntax, decoded budgets, count satisfaction and EOF are runtime
        # constraints; contentEncoding alone is not a binary validator.


if __name__ == "__main__":
    unittest.main()
