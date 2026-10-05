"""Independent validation of the public Models operation schemas."""
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator, FormatChecker

DOCUMENT = json.loads((Path(__file__).resolve().parents[2] / "docs/openapi.json").read_text())


def validator(name):
    return Draft202012Validator({
        "$ref": f"#/components/schemas/{name}",
        "components": DOCUMENT["components"],
    }, format_checker=FormatChecker())


class ModelsSchemaTests(unittest.TestCase):
    def test_model_required_types_and_unreported_optional_field(self):
        model = {"id": "synthetic-model", "object": "model", "created": 7,
                 "owned_by": "Synthetic Developer"}
        check = validator("Model")
        check.validate(model)
        check.validate({**model, "shutdown_date": None})
        check.validate({**model, "shutdown_date": "2027-01-02"})
        for key in model:
            missing = {k: v for k, v in model.items() if k != key}
            self.assertFalse(check.is_valid(missing))
            self.assertFalse(check.is_valid({**model, key: None}))
        for extra in (
            {"object": "list"}, {"id": "provider/model"}, {"created": True},
            {"created": 0}, {"created": -1}, {"created": 1.5}, {"created": "7"},
            {"owned_by": ""}, {"shutdown_date": "unknown"}, {"price": 1},
            {"credential": "synthetic"}, {"_openbridge": {}},
        ):
            self.assertFalse(check.is_valid({**model, **extra}))

    def test_list_empty_count_and_standard_nonpagination(self):
        check = validator("ModelList")
        model = {"id": "synthetic-model", "object": "model", "created": 7,
                 "owned_by": "Synthetic Developer"}
        check.validate({"object": "list", "data": []})
        check.validate({"object": "list", "data": [model] * 64})
        for value in (
            {"object": "list"}, {"data": []}, {"object": "model", "data": []},
            {"object": "list", "data": None}, {"object": "list", "data": [{}]},
            {"object": "list", "data": [model] * 65},
            {"object": "list", "data": [], "has_more": False},
        ):
            self.assertFalse(check.is_valid(value))

    def test_paths_auth_and_deletion_denial_match_public_contract(self):
        self.assertEqual(DOCUMENT["security"], [{"bearerAuth": []}])
        paths = DOCUMENT["paths"]
        self.assertEqual(paths["/v1/models"]["get"]["responses"]["200"]["content"]
                         ["application/json"]["schema"]["$ref"], "#/components/schemas/ModelList")
        single = paths["/v1/models/{model}"]
        self.assertEqual(single["parameters"][0]["name"], "model")
        self.assertTrue(single["parameters"][0]["required"])
        self.assertEqual(single["get"]["responses"]["200"]["content"]
                         ["application/json"]["schema"]["$ref"], "#/components/schemas/Model")
        self.assertNotIn("200", single["delete"]["responses"])
        for method in (paths["/v1/models"]["get"], single["get"], single["delete"]):
            self.assertIn("401", method["responses"])
            self.assertIn("503", method["responses"])
        for status in ("403", "404"):
            self.assertIn(status, single["delete"]["responses"])


if __name__ == "__main__":
    unittest.main()
