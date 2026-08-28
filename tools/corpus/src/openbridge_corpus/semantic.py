"""Validate normalized traces against protocol-neutral semantic oracles."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker

from .corpuslib import (
    CorpusError,
    SemanticCase,
    _loads_bounded_json,
    _schema_validator,
    discover_semantic_cases,
)


def _safe_trace_schema_errors(
    validator: Draft202012Validator, trace: Any
) -> list[str]:
    """Return schema paths without including instance values or validator messages."""
    errors: list[str] = []
    for error in sorted(validator.iter_errors(trace), key=lambda item: list(item.path)):
        location = ".".join(str(part) for part in error.path) or "(root)"
        errors.append(f"semantic trace: {location}: does not satisfy the trace schema")
    return errors


def find_semantic_case(root: Path, case_id: str) -> SemanticCase:
    """Return exactly one semantic case by ID or raise ``CorpusError``."""
    matches = [
        case for case in discover_semantic_cases(root) if case.case_id == case_id
    ]
    if len(matches) != 1:
        raise CorpusError(
            f"semantic case {case_id!r} matched {len(matches)} canonical manifests"
        )
    return matches[0]


def _arguments_contain(actual: Any, expected: Any) -> bool:
    """Return whether actual recursively contains the expected JSON projection."""
    if isinstance(expected, dict):
        return isinstance(actual, dict) and all(
            key in actual and _arguments_contain(actual[key], value)
            for key, value in expected.items()
        )
    if isinstance(expected, list):
        return isinstance(actual, list) and len(actual) == len(expected) and all(
            _arguments_contain(actual_item, expected_item)
            for actual_item, expected_item in zip(actual, expected)
        )
    return type(actual) is type(expected) and actual == expected


def _json_equal(actual: Any, expected: Any) -> bool:
    """Compare JSON values recursively without Python bool/number coercion."""
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return actual.keys() == expected.keys() and all(
            _json_equal(actual[key], expected[key]) for key in expected
        )
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            _json_equal(actual_item, expected_item)
            for actual_item, expected_item in zip(actual, expected)
        )
    return actual == expected


def _call_matches(expected: dict[str, Any], actual: dict[str, Any]) -> bool:
    """Return whether one normalized call satisfies an expected call matcher."""
    if expected["name"] != actual["name"]:
        return False
    if expected["arguments_match"] == "exact":
        return _json_equal(actual["arguments"], expected["arguments"])
    return _arguments_contain(actual["arguments"], expected["arguments"])


def _match_exact_calls(
    required: list[dict[str, Any]],
    actual: list[dict[str, Any]],
    allow_additional: bool,
) -> tuple[list[str], dict[int, int]]:
    """Compare required calls in order and return errors plus required-to-actual assignments."""
    errors: list[str] = []
    assignment: dict[int, int] = {}

    # Compare aligned calls without exposing tool names or argument values.
    for index, expected in enumerate(required):
        if index >= len(actual):
            errors.append(f"required tool call is missing at required_calls[{index}]")
            continue
        assignment[index] = index
        observed = actual[index]
        if expected["name"] != observed["name"]:
            errors.append(
                f"tool_calls[{index}].name differs from required_calls[{index}]"
            )
        elif not _call_matches(expected, observed):
            errors.append(
                f"required_calls[{index}].arguments differ from tool_calls[{index}]"
            )

    # Reject trailing calls unless this oracle explicitly permits additions.
    if not allow_additional:
        for index in range(len(required), len(actual)):
            errors.append(f"unexpected tool call at tool_calls[{index}]")
    return errors, assignment


def _find_any_order_assignment(
    required: list[dict[str, Any]], actual: list[dict[str, Any]]
) -> dict[int, int]:
    """Find a complete deterministic bipartite assignment for unordered calls."""
    candidates = {
        required_index: [
            actual_index
            for actual_index, observed in enumerate(actual)
            if _call_matches(expected, observed)
        ]
        for required_index, expected in enumerate(required)
    }
    assignment: dict[int, int] = {}

    # Match the most constrained expectation first and backtrack across duplicates.
    order = sorted(candidates, key=lambda index: (len(candidates[index]), index))

    def assign(position: int, used: set[int]) -> bool:
        """Assign one expectation recursively without reusing actual calls."""
        if position == len(order):
            return True
        required_index = order[position]
        for actual_index in candidates[required_index]:
            if actual_index in used:
                continue
            assignment[required_index] = actual_index
            if assign(position + 1, used | {actual_index}):
                return True
            del assignment[required_index]
        return False

    if not assign(0, set()):
        assignment.clear()
        used: set[int] = set()
        for required_index in range(len(required)):
            for actual_index in candidates[required_index]:
                if actual_index not in used:
                    assignment[required_index] = actual_index
                    used.add(actual_index)
                    break
    return assignment


def _match_any_order_calls(
    required: list[dict[str, Any]],
    actual: list[dict[str, Any]],
    allow_additional: bool,
) -> tuple[list[str], dict[int, int]]:
    """Compare required calls as a multiset and return errors plus assignments."""
    errors: list[str] = []
    assignment = _find_any_order_assignment(required, actual)
    matched = set(assignment.values())

    # Distinguish a missing tool from a same-name call with wrong arguments.
    for index, expected in enumerate(required):
        if index in assignment:
            continue
        if any(observed["name"] == expected["name"] for observed in actual):
            errors.append(
                f"required_calls[{index}].arguments have no matching tool call"
            )
        else:
            errors.append(f"required tool call is missing at required_calls[{index}]")

    # Reject every unmatched call when additions are outside the oracle.
    if not allow_additional:
        for index in range(len(actual)):
            if index not in matched:
                errors.append(f"unexpected tool call at tool_calls[{index}]")
    return errors, assignment


def _verify_trace_integrity(
    events: list[dict[str, Any]], errors: list[str]
) -> tuple[list[dict[str, Any]], list[dict[str, Any]], str | None]:
    """Validate event ordering and identity, returning calls, results, and final text."""
    calls: list[dict[str, Any]] = []
    results: list[dict[str, Any]] = []
    call_ids: set[str] = set()
    result_ids: set[str] = set()
    final_text: str | None = None
    previous_turn = -1

    # Walk the normalized timeline and validate causal identity without interpreting output bodies.
    for event_index, event in enumerate(events):
        turn = event["turn"]
        if turn < previous_turn:
            errors.append(f"events[{event_index}].turn moves backwards")
        previous_turn = max(previous_turn, turn)
        if event["type"] == "assistant_tool_call":
            call_index = len(calls)
            if event["call_id"] in call_ids:
                errors.append(f"tool_calls[{call_index}].call_id is duplicated")
            call_ids.add(event["call_id"])
            calls.append(event)
        elif event["type"] == "tool_result":
            if event["call_id"] not in call_ids:
                errors.append(f"events[{event_index}].call_id has no preceding tool call")
            if event["call_id"] in result_ids:
                errors.append(f"events[{event_index}].call_id has a duplicate result")
            result_ids.add(event["call_id"])
            results.append(event)
        elif event["type"] == "assistant_message":
            if final_text is not None:
                errors.append(f"events[{event_index}] repeats the final assistant message")
            final_text = event["text"]
            if event_index != len(events) - 1:
                errors.append(f"events[{event_index}] is not the final trace event")
    return calls, results, final_text


def _verify_results(
    oracle: dict[str, Any],
    assignment: dict[int, int],
    calls: list[dict[str, Any]],
    results: list[dict[str, Any]],
    errors: list[str],
) -> None:
    """Match required tool outputs through assigned calls without exposing values."""
    results_by_call_id = {
        result["call_id"]: (index, result) for index, result in enumerate(results)
    }
    matched_results: set[int] = set()

    # Resolve each expected output through the actual call ID selected by call matching.
    for index, expected in enumerate(oracle["required"]):
        actual_call_index = assignment.get(expected["required_call_index"])
        if actual_call_index is None:
            continue
        observed_entry = results_by_call_id.get(calls[actual_call_index]["call_id"])
        if observed_entry is None:
            errors.append(f"required_results[{index}] is missing")
            continue
        result_index, observed = observed_entry
        matched_results.add(result_index)
        if expected["output_match"] == "exact":
            matches = _json_equal(observed["output"], expected["output"])
        else:
            matches = _arguments_contain(observed["output"], expected["output"])
        if not matches:
            errors.append(
                f"required_results[{index}].output differs from tool_results[{result_index}]"
            )

    # Reject results not consumed by the oracle when the case closes that boundary.
    if not oracle["allow_additional"]:
        for index in range(len(results)):
            if index not in matched_results:
                errors.append(f"unexpected tool result at tool_results[{index}]")


def _verify_argument_schemas(
    case: SemanticCase,
    calls: list[dict[str, Any]],
    errors: list[str],
) -> None:
    """Validate normalized call arguments against their declared function schemas."""
    tools = {
        tool["name"]: tool for tool in case.data["task"].get("tools", [])
    }

    # Validate only declared tools and report schema locations without values.
    for index, call in enumerate(calls):
        tool = tools.get(call["name"])
        if tool is None:
            errors.append(f"tool_calls[{index}].name is not declared")
            continue
        validator = Draft202012Validator(
            tool["parameters"], format_checker=FormatChecker()
        )
        for schema_error in sorted(
            validator.iter_errors(call["arguments"]),
            key=lambda item: list(item.path),
        ):
            location = "".join(f"[{part!r}]" for part in schema_error.path)
            errors.append(
                f"tool_calls[{index}].arguments{location} fails the tool schema"
            )


def _verify_final_response(
    case: SemanticCase, final_text: str | None, errors: list[str]
) -> None:
    """Validate structured output and fixed final-response facts without exposing content."""
    oracle = case.data["oracle"]["final_response"]
    if oracle["required"] and final_text is None:
        errors.append("final_response is required")
        return
    if final_text is None:
        return

    task = case.data["task"]
    if task.get("kind") == "structured":
        try:
            structured = _loads_bounded_json(final_text)
        except CorpusError:
            errors.append("final_response is not valid structured JSON")
        else:
            validator = Draft202012Validator(
                task["response_format"]["schema"], format_checker=FormatChecker()
            )
            for schema_error in sorted(
                validator.iter_errors(structured), key=lambda item: list(item.path)
            ):
                location = "".join(f"[{part!r}]" for part in schema_error.path)
                errors.append(
                    f"final_response structured output{location} fails the response schema"
                )

    # Normalize case only for matching; never include the compared text in diagnostics.
    observed = final_text if oracle["case_sensitive"] else final_text.casefold()
    for index, expected in enumerate(oracle["contains_all"]):
        candidate = expected if oracle["case_sensitive"] else expected.casefold()
        if candidate not in observed:
            errors.append(f"final_response.contains_all[{index}] is missing")
    for index, forbidden in enumerate(oracle["contains_none"]):
        candidate = forbidden if oracle["case_sensitive"] else forbidden.casefold()
        if candidate in observed:
            errors.append(f"final_response.contains_none[{index}] is present")


def verify_semantic_trace(
    root: Path, case_id: str, trace: dict[str, Any]
) -> list[str]:
    """Compare a normalized trace with one semantic oracle and return safe errors.

    Raise ``CorpusError`` when the semantic case is missing or the trace does not satisfy the
    normalized trace schema. An empty list means the trace satisfies this deterministic boundary.
    """
    root = root.resolve()

    # Load the canonical oracle and reject structurally invalid trace documents.
    case = find_semantic_case(root, case_id)
    validator = _schema_validator(root, "semantic-trace")
    schema_errors = _safe_trace_schema_errors(validator, trace)
    if schema_errors:
        raise CorpusError(
            "semantic trace schema validation failed:\n" + "\n".join(schema_errors)
        )
    errors: list[str] = []
    if trace["case_id"] != case.case_id:
        errors.append("semantic_trace.case_id differs")

    # Validate event identity and every actual argument object against its function schema.
    calls, results, final_text = _verify_trace_integrity(trace["events"], errors)
    _verify_argument_schemas(case, calls, errors)

    # Match expected calls using the oracle's ordered or unordered policy.
    call_oracle = case.data["oracle"]["calls"]
    if call_oracle["match_order"] == "exact":
        match_errors, assignment = _match_exact_calls(
            call_oracle["required"], calls, call_oracle["allow_additional"]
        )
    else:
        match_errors, assignment = _match_any_order_calls(
            call_oracle["required"], calls, call_oracle["allow_additional"]
        )
    errors.extend(match_errors)
    forbidden_names = set(call_oracle["forbidden_names"])
    for index, call in enumerate(calls):
        if call["name"] in forbidden_names:
            errors.append(f"tool_calls[{index}].name is forbidden")

    # Match tool outputs through call assignments, then validate final response facts.
    _verify_results(
        case.data["oracle"]["results"], assignment, calls, results, errors
    )
    _verify_final_response(case, final_text, errors)

    # Stabilize duplicate diagnostics for CLI consumers.
    return list(dict.fromkeys(errors))
