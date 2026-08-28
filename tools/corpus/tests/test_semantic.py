"""Validate protocol-neutral semantic cases, plans, and trace verdicts."""

from __future__ import annotations

import copy
import json
import shutil
from pathlib import Path

import pytest

from openbridge_corpus.cli import main
from openbridge_corpus.corpuslib import (
    CorpusError,
    discover_semantic_cases,
    lint_corpus,
    load_json,
)
from openbridge_corpus.plans import validate_runtime_document
from openbridge_corpus.semantic import _json_equal, verify_semantic_trace
from openbridge_corpus.semantic_plan import build_semantic_plan


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CORPUS_ROOT = REPOSITORY_ROOT / "testdata"


def _semantic_case(case_id: str):
    """Return one canonical semantic case by stable ID."""
    return next(
        case
        for case in discover_semantic_cases(CORPUS_ROOT)
        if case.case_id == case_id
    )


def _reference_trace(case_id: str) -> dict:
    """Load the canonical reference trace declared by one semantic case."""
    case = _semantic_case(case_id)
    return load_json(case.directory / case.data["artifacts"]["reference_trace"])


def _copy_corpus(destination: Path) -> Path:
    """Copy canonical inputs without derived outputs for destructive lint tests."""
    target = destination / "testdata"
    shutil.copytree(
        CORPUS_ROOT,
        target,
        ignore=shutil.ignore_patterns("generated", "reports", "dist", "runtime"),
    )
    return target


def test_repository_semantic_cases_are_complete_and_protocol_neutral() -> None:
    """Verify the semantic corpus covers tools, context use, and structured output."""
    cases = discover_semantic_cases(CORPUS_ROOT)
    assert len(cases) == 14
    assert {case.case_id for case in cases} == {
        "context.associative_retrieval",
        "context.conflict_resolution",
        "context.literal_retrieval",
        "context.multi_fact_integration",
        "function.ambiguous_selection",
        "function.forced_tool",
        "function.missing_argument_clarification",
        "function.no_tool_needed",
        "function.parallel_independent",
        "function.result_grounding",
        "function.single_tool_arguments",
        "function.tool_choice_none",
        "function.tool_choice_required",
        "structured.strict_nested_json",
    }
    required_targets = {
        "chat_native",
        "responses_native",
        "chat_to_responses",
        "responses_to_chat",
    }
    assert all(set(case.data["applies_to"]) == required_targets for case in cases)


@pytest.mark.parametrize("placement", ["start", "middle", "end"])
def test_context_semantic_plan_is_deterministic_and_exact_size(
    placement: str,
) -> None:
    """Build the same bounded context prompt deterministically at every needle position."""
    first = build_semantic_plan(
        CORPUS_ROOT,
        "context.literal_retrieval",
        target_bytes=4096,
        placement=placement,
    )
    second = build_semantic_plan(
        CORPUS_ROOT,
        "context.literal_retrieval",
        target_bytes=4096,
        placement=placement,
    )

    assert first == second
    assert first["role"] == "semantic_execution_plan"
    assert first["task"]["kind"] == "context"
    assert first["target_utf8_bytes"] == 4096
    assert first["actual_utf8_bytes"] == 4096
    assert len(first["task"]["prompt"].encode("utf-8")) == 4096
    needle_line = first["task"]["prompt"].index("The cobalt archive code")
    needle_index = first["task"]["prompt"].index("Q7M4")
    assert first["task"]["prompt"][needle_line - 1] == "\n"
    if placement == "start":
        assert needle_index < 512
    elif placement == "middle":
        assert 1024 < needle_index < 3072
    else:
        assert needle_index > 3584


def test_context_semantic_plan_rejects_undeclared_axes() -> None:
    """Reject unreviewed length or placement values before producing runtime prompts."""
    with pytest.raises(CorpusError):
        build_semantic_plan(
            CORPUS_ROOT,
            "context.literal_retrieval",
            target_bytes=8192,
            placement="middle",
        )
    with pytest.raises(CorpusError):
        build_semantic_plan(
            CORPUS_ROOT,
            "context.literal_retrieval",
            target_bytes=4096,
            placement="unsupported",
        )


def test_semantic_plan_validates_case_schema_before_context_allocation(
    tmp_path: Path,
) -> None:
    """Reject an over-limit canonical target before allocating its prompt."""
    root = _copy_corpus(tmp_path)
    case_path = next(
        (root / "semantic-cases").rglob("context.literal_retrieval/case.json")
    )
    case = load_json(case_path)
    case["task"]["context"]["target_bytes"] = [9_000_000]
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    with pytest.raises(CorpusError, match="does not satisfy semantic-case schema"):
        build_semantic_plan(
            root,
            "context.literal_retrieval",
            target_bytes=9_000_000,
            placement="middle",
        )


def test_non_context_plan_rejects_context_axes() -> None:
    """Keep function and structured plans free of misleading length metadata."""
    plan = build_semantic_plan(CORPUS_ROOT, "function.no_tool_needed")
    validate_runtime_document(CORPUS_ROOT, "semantic-plan", plan)
    plan["target_utf8_bytes"] = 4096
    with pytest.raises(CorpusError):
        validate_runtime_document(CORPUS_ROOT, "semantic-plan", plan)

    malformed = build_semantic_plan(CORPUS_ROOT, "function.single_tool_arguments")
    malformed["task"]["prompt"] = "SENSITIVE_RUNTIME_PROMPT"
    malformed["task"]["controls"] = {}
    malformed["task"]["tools"] = [{}]
    with pytest.raises(CorpusError) as captured:
        validate_runtime_document(CORPUS_ROOT, "semantic-plan", malformed)
    assert "SENSITIVE_RUNTIME_PROMPT" not in str(captured.value)


def test_exact_json_matching_distinguishes_boolean_and_numeric_values() -> None:
    """Keep JSON exact matching stricter than Python's bool/int equality."""
    assert not _json_equal({"value": 1}, {"value": True})
    assert not _json_equal({"value": 1}, {"value": 1.0})
    assert _json_equal({"value": [1, False]}, {"value": [1, False]})


def test_context_semantic_plan_cli_writes_valid_plan_without_printing_prompt(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Compile one plan without printing its prompt outside the runtime artifact."""
    root = _copy_corpus(tmp_path)
    assert (
        main(
            [
                "--root",
                str(root),
                "build-semantic-plan",
                "--case",
                "context.literal_retrieval",
                "--target-bytes",
                "4096",
                "--placement",
                "middle",
            ]
        )
        == 0
    )
    captured = capsys.readouterr()
    assert "wrote" in captured.out
    assert "Q7M4" not in captured.out
    plan = load_json(
        root / "runtime" / "context.literal_retrieval.semantic-plan.json"
    )
    assert plan["actual_utf8_bytes"] == 4096
    assert len(plan["task"]["prompt"].encode("utf-8")) == 4096


def test_structured_semantic_verifier_requires_valid_schema_output() -> None:
    """Separate valid strict JSON from malformed or schema-invalid assistant text."""
    case_id = "structured.strict_nested_json"
    matching = _reference_trace(case_id)
    assert verify_semantic_trace(CORPUS_ROOT, case_id, matching) == []

    malformed = copy.deepcopy(matching)
    malformed["events"][-1]["text"] = "SENSITIVE_NOT_JSON"
    errors = verify_semantic_trace(CORPUS_ROOT, case_id, malformed)
    assert any("structured JSON" in error for error in errors)
    assert all("SENSITIVE_NOT_JSON" not in error for error in errors)

    invalid = copy.deepcopy(matching)
    invalid["events"][-1]["text"] = json.dumps(
        {"inventory": {"quantity": "twelve", "warehouse": "east"}}
    )
    errors = verify_semantic_trace(CORPUS_ROOT, case_id, invalid)
    assert any("structured output" in error for error in errors)
    assert all("twelve" not in error for error in errors)

    for invalid_json in [
        '{"inventory":{"quantity":12,"quantity":12,"warehouse":"east"}}',
        '{"inventory":{"quantity":NaN,"warehouse":"east"}}',
        "[" * 5000 + "0" + "]" * 5000,
    ]:
        ambiguous = copy.deepcopy(matching)
        ambiguous["events"][-1]["text"] = invalid_json
        errors = verify_semantic_trace(CORPUS_ROOT, case_id, ambiguous)
        assert any("valid structured JSON" in error for error in errors)
        assert all(invalid_json not in error for error in errors)


def test_semantic_trace_event_count_is_bounded() -> None:
    """Reject oversized normalized event streams before semantic matching."""
    trace = {
        "schema_version": "0.1",
        "case_id": "function.no_tool_needed",
        "events": [
            {"type": "assistant_message", "turn": index, "text": "bounded"}
            for index in range(4097)
        ],
    }

    with pytest.raises(CorpusError) as captured:
        verify_semantic_trace(CORPUS_ROOT, "function.no_tool_needed", trace)

    assert "does not satisfy the trace schema" in str(captured.value)
    assert "bounded" not in str(captured.value)


def test_every_reference_trace_satisfies_its_semantic_oracle() -> None:
    """Verify every checked-in reference trace is a positive deterministic example."""
    for case in discover_semantic_cases(CORPUS_ROOT):
        trace = load_json(case.directory / case.data["artifacts"]["reference_trace"])
        assert verify_semantic_trace(CORPUS_ROOT, case.case_id, trace) == []


def test_semantic_verifier_rejects_wrong_missing_and_additional_calls() -> None:
    """Verify call-set mismatches produce stable diagnostics without argument bodies."""
    case_id = "function.single_tool_arguments"
    matching = _reference_trace(case_id)

    wrong = copy.deepcopy(matching)
    wrong["events"][0]["name"] = "SENSITIVE_TOOL_SENTINEL"
    wrong_errors = verify_semantic_trace(CORPUS_ROOT, case_id, wrong)
    assert any("tool_calls[0].name" in error for error in wrong_errors)
    assert all("SENSITIVE_TOOL_SENTINEL" not in error for error in wrong_errors)

    missing = copy.deepcopy(matching)
    missing["events"] = []
    assert any(
        "required tool call" in error
        for error in verify_semantic_trace(CORPUS_ROOT, case_id, missing)
    )

    additional = copy.deepcopy(matching)
    additional["events"].append(
        {
            "arguments": {"location": "Osaka", "unit": "celsius"},
            "call_id": "call_extra",
            "name": "get_weather",
            "turn": 0,
            "type": "assistant_tool_call",
        }
    )
    assert any(
        "unexpected tool call" in error
        for error in verify_semantic_trace(CORPUS_ROOT, case_id, additional)
    )


def test_semantic_verifier_checks_argument_schema_and_expected_values() -> None:
    """Verify argument validity and exact expected values are separate deterministic checks."""
    case_id = "function.single_tool_arguments"
    invalid = _reference_trace(case_id)
    invalid["events"][0]["arguments"]["unit"] = "kelvin"

    errors = verify_semantic_trace(CORPUS_ROOT, case_id, invalid)

    assert any("tool_calls[0].arguments" in error for error in errors)
    assert any("required_calls[0].arguments" in error for error in errors)
    assert all("kelvin" not in error for error in errors)

    malformed = _reference_trace(case_id)
    malformed["events"][0]["arguments"] = "SENSITIVE_SCHEMA_SENTINEL"
    with pytest.raises(CorpusError) as captured:
        verify_semantic_trace(CORPUS_ROOT, case_id, malformed)
    assert "SENSITIVE_SCHEMA_SENTINEL" not in str(captured.value)


def test_semantic_verifier_accepts_unordered_parallel_calls() -> None:
    """Verify independent parallel calls match as a set instead of by emission order."""
    case_id = "function.parallel_independent"
    trace = _reference_trace(case_id)
    call_names = [
        event["name"]
        for event in trace["events"]
        if event["type"] == "assistant_tool_call"
    ]
    oracle_names = [
        call["name"]
        for call in _semantic_case(case_id).data["oracle"]["calls"]["required"]
    ]

    assert call_names == list(reversed(oracle_names))
    assert verify_semantic_trace(CORPUS_ROOT, case_id, trace) == []


def test_semantic_verifier_enforces_no_tool_and_final_response_facts() -> None:
    """Verify forbidden calls and grounded final-answer facts are independently enforced."""
    no_tool_id = "function.no_tool_needed"
    unexpected_call = _reference_trace(no_tool_id)
    unexpected_call["events"].insert(
        0,
        {
            "arguments": {"location": "Shanghai", "unit": "celsius"},
            "call_id": "call_unexpected",
            "name": "get_weather",
            "turn": 0,
            "type": "assistant_tool_call",
        },
    )
    assert any(
        "unexpected tool call" in error
        for error in verify_semantic_trace(CORPUS_ROOT, no_tool_id, unexpected_call)
    )

    grounding_id = "function.result_grounding"
    missing_result = _reference_trace(grounding_id)
    missing_result["events"] = [
        event for event in missing_result["events"] if event["type"] != "tool_result"
    ]
    assert any(
        "required_results[0] is missing" in error
        for error in verify_semantic_trace(CORPUS_ROOT, grounding_id, missing_result)
    )

    wrong_result = _reference_trace(grounding_id)
    wrong_result["events"][1]["output"]["quantity"] = 999
    result_errors = verify_semantic_trace(CORPUS_ROOT, grounding_id, wrong_result)
    assert any("required_results[0].output" in error for error in result_errors)
    assert all("999" not in error for error in result_errors)

    nonfinal_message = _reference_trace(grounding_id)
    message = nonfinal_message["events"].pop()
    message["turn"] = 1
    nonfinal_message["events"][1]["turn"] = 2
    nonfinal_message["events"].insert(1, message)
    assert any(
        "is not the final trace event" in error
        for error in verify_semantic_trace(CORPUS_ROOT, grounding_id, nonfinal_message)
    )

    ungrounded = _reference_trace(grounding_id)
    ungrounded["events"][-1]["text"] = "SENSITIVE_FINAL_SENTINEL"
    errors = verify_semantic_trace(CORPUS_ROOT, grounding_id, ungrounded)
    assert any("final_response.contains_all" in error for error in errors)
    assert all("SENSITIVE_FINAL_SENTINEL" not in error for error in errors)


def test_semantic_lint_rejects_catalog_and_strict_schema_drift(
    tmp_path: Path,
) -> None:
    """Verify semantic discovery and strict function schemas remain canonical lint contracts."""
    root = _copy_corpus(tmp_path)

    # Remove one semantic catalog declaration.
    catalog_path = root / "catalog.json"
    catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
    catalog["semantic_case_ids"] = catalog["semantic_case_ids"][1:]
    catalog_path.write_text(
        json.dumps(catalog, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    # Break the strict root object guarantee in one function definition.
    case_path = next(
        (root / "semantic-cases").rglob("function.single_tool_arguments/case.json")
    )
    case = json.loads(case_path.read_text(encoding="utf-8"))
    case["task"]["tools"][0]["parameters"]["additionalProperties"] = True
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    errors = lint_corpus(root)

    assert any("catalog/semantic-case mismatch" in error for error in errors)
    assert any("strict tool schema" in error for error in errors)


def test_semantic_lint_rejects_invalid_context_recipe(tmp_path: Path) -> None:
    """Reject context generators that cannot produce stable bounded ASCII distractors."""
    root = _copy_corpus(tmp_path)
    case_path = next(
        (root / "semantic-cases").rglob("context.literal_retrieval/case.json")
    )
    case = json.loads(case_path.read_text(encoding="utf-8"))
    case["task"]["context"]["distractor_template"] = "missing placeholders"
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    errors = lint_corpus(root)

    assert any("distractor_template" in error for error in errors)

    case["task"]["context"]["distractor_template"] = "{index[0]} {token}"
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    errors = lint_corpus(root)
    assert any("invalid context recipe" in error for error in errors)


def test_semantic_lint_checks_conditional_strict_schema_branches(
    tmp_path: Path,
) -> None:
    """Reject open objects hidden in conditional strict-output branches."""
    root = _copy_corpus(tmp_path)
    case_path = next(
        (root / "semantic-cases").rglob("structured.strict_nested_json/case.json")
    )
    case = load_json(case_path)
    response_schema = case["task"]["response_format"]["schema"]
    response_schema["if"] = {"properties": {"inventory": {"type": "object"}}}
    response_schema["then"] = {"properties": {"extra": {"type": "string"}}}
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    errors = lint_corpus(root)

    assert any("strict response schema" in error and ".then" in error for error in errors)


def test_semantic_lint_schema_diagnostics_do_not_echo_instance_values(
    tmp_path: Path,
) -> None:
    """Report schema paths and rules without reflecting malformed task content."""
    root = _copy_corpus(tmp_path)
    case_path = next(
        (root / "semantic-cases").rglob("structured.strict_nested_json/case.json")
    )
    case = load_json(case_path)
    case["task"]["response_format"]["schema"] = "SENSITIVE_SCHEMA_VALUE"
    case_path.write_text(
        json.dumps(case, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    errors = lint_corpus(root)

    assert any("schema rule" in error for error in errors)
    assert all("SENSITIVE_SCHEMA_VALUE" not in error for error in errors)


def test_semantic_cli_reports_safe_pass_and_failure_verdicts(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Verify the CLI exposes verdicts without echoing trace content."""
    case_id = "function.result_grounding"
    matching_path = tmp_path / "matching.json"
    matching_path.write_text(
        json.dumps(_reference_trace(case_id), ensure_ascii=False),
        encoding="utf-8",
    )
    assert (
        main(
            [
                "--root",
                str(CORPUS_ROOT),
                "verify-semantic-trace",
                "--case",
                case_id,
                "--trace",
                str(matching_path),
            ]
        )
        == 0
    )
    captured = capsys.readouterr()
    assert "semantic trace passed" in captured.out
    assert captured.err == ""

    failing = _reference_trace(case_id)
    failing["events"][-1]["text"] = "SENSITIVE_CLI_SENTINEL"
    failing_path = tmp_path / "failing.json"
    failing_path.write_text(json.dumps(failing), encoding="utf-8")
    assert (
        main(
            [
                "--root",
                str(CORPUS_ROOT),
                "verify-semantic-trace",
                "--case",
                case_id,
                "--trace",
                str(failing_path),
            ]
        )
        == 1
    )
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "semantic trace failed" in captured.err
    assert "SENSITIVE_CLI_SENTINEL" not in captured.err
