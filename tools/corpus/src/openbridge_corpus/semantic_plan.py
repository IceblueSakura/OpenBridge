"""Compile credential-free semantic cases into deterministic runtime execution plans."""

from __future__ import annotations

import hashlib
from copy import deepcopy
from pathlib import Path
from string import Formatter
from typing import Any

from .corpuslib import CorpusError, _schema_validator
from .semantic import find_semantic_case


_CONTEXT_PLACEMENTS = {"start", "middle", "end"}
_INDEX_FORMATS = {"", "d", "06d"}


def _validate_distractor_template(template: str) -> None:
    """Allow only bounded ``index`` and ``token`` replacements in corpus templates."""
    seen: set[str] = set()
    try:
        parts = list(Formatter().parse(template))
    except ValueError as error:
        raise CorpusError("context distractor_template has invalid format syntax") from error
    for _, field_name, format_spec, conversion in parts:
        if field_name is None:
            continue
        if field_name not in {"index", "token"} or conversion is not None:
            raise CorpusError("context distractor_template uses an unsupported field")
        if field_name == "index" and format_spec not in _INDEX_FORMATS:
            raise CorpusError("context distractor_template uses an unsupported index format")
        if field_name == "token" and format_spec:
            raise CorpusError("context distractor_template cannot format the token")
        seen.add(field_name)
    if seen != {"index", "token"}:
        raise CorpusError("context distractor_template must include index and token")


def _semantic_task_kind(task: dict[str, Any]) -> str:
    """Return the explicit task kind, preserving 0.7 function cases without a kind field."""
    if "tools" in task:
        return "function"
    kind = task.get("kind")
    if kind in {"context", "structured"}:
        return str(kind)
    raise CorpusError("semantic task has no recognized kind")


def _distractor_bytes(template: str, seed: int, required: int) -> bytes:
    """Generate at least ``required`` deterministic ASCII distractor bytes."""
    _validate_distractor_template(template)
    chunks: list[bytes] = []
    size = 0
    index = 0
    while size < required:
        token = hashlib.sha256(f"{seed}:{index}".encode()).hexdigest()[:12]
        try:
            record = template.format(index=index, token=token)
        except (AttributeError, IndexError, KeyError, OverflowError, TypeError, ValueError) as error:
            raise CorpusError("context distractor_template cannot be formatted") from error
        try:
            encoded = record.encode("ascii")
        except UnicodeEncodeError as error:
            raise CorpusError("context distractor_template must render ASCII") from error
        if not encoded:
            raise CorpusError("context distractor_template rendered an empty record")
        chunks.append(encoded)
        size += len(encoded)
        index += 1
    return b"".join(chunks)[:required]


def _build_context_prompt(
    task: dict[str, Any], target_bytes: int | None, placement: str | None
) -> tuple[str, int, str, int]:
    """Build one exact-size synthetic prompt from a reviewed context recipe."""
    recipe = task["context"]
    declared_targets = recipe["target_bytes"]
    declared_placements = recipe["placements"]
    selected_target = target_bytes if target_bytes is not None else declared_targets[0]
    selected_placement = placement if placement is not None else declared_placements[0]
    if selected_target not in declared_targets:
        raise CorpusError("target_bytes is not declared by the semantic case")
    if selected_placement not in _CONTEXT_PLACEMENTS or selected_placement not in declared_placements:
        raise CorpusError("placement is not declared by the semantic case")

    prefix = f"{task['instruction']}\n\nCONTEXT\n"
    needle = f"\n{recipe['needle']}"
    suffix = f"\nQUESTION\n{task['question']}\n"
    try:
        prefix_bytes = prefix.encode("ascii")
        needle_bytes = needle.encode("ascii")
        suffix_bytes = suffix.encode("ascii")
    except UnicodeEncodeError as error:
        raise CorpusError("context task generation fields must be ASCII") from error
    minimum = len(prefix_bytes) + len(needle_bytes) + len(suffix_bytes)
    if selected_target < minimum:
        raise CorpusError("target_bytes is smaller than the semantic prompt minimum")

    filler = _distractor_bytes(
        recipe["distractor_template"], recipe["seed"], selected_target - minimum
    )
    if selected_placement == "start":
        before, after = b"", filler
    elif selected_placement == "middle":
        midpoint = len(filler) // 2
        split = filler.rfind(b"\n", 0, midpoint + 1) + 1
        if split == 0:
            split = midpoint
        before, after = filler[:split], filler[split:]
    else:
        before, after = filler, b""
    prompt_bytes = prefix_bytes + before + needle_bytes + after + suffix_bytes
    if len(prompt_bytes) != selected_target:
        raise CorpusError("semantic context compiler produced an unexpected byte length")
    return prompt_bytes.decode("ascii"), selected_target, selected_placement, recipe["seed"]


def build_semantic_plan(
    root: Path,
    case_id: str,
    *,
    target_bytes: int | None = None,
    placement: str | None = None,
) -> dict[str, Any]:
    """Compile one canonical semantic case without network, credentials, or model selection."""
    case = find_semantic_case(root.resolve(), case_id)
    validator = _schema_validator(root.resolve(), "semantic-case")
    if next(validator.iter_errors(case.data), None) is not None:
        raise CorpusError("semantic case does not satisfy semantic-case schema")
    task = case.data["task"]
    kind = _semantic_task_kind(task)
    plan: dict[str, Any] = {
        "applies_to": deepcopy(case.data["applies_to"]),
        "case_id": case.case_id,
        "role": "semantic_execution_plan",
        "schema_version": "0.1",
    }
    if kind == "context":
        prompt, selected_target, selected_placement, seed = _build_context_prompt(
            task, target_bytes, placement
        )
        plan.update(
            {
                "actual_utf8_bytes": len(prompt.encode("utf-8")),
                "placement": selected_placement,
                "seed": seed,
                "target_utf8_bytes": selected_target,
                "task": {"kind": "context", "prompt": prompt},
            }
        )
        return plan
    if target_bytes is not None or placement is not None:
        raise CorpusError("length and placement axes apply only to context semantic cases")
    if kind == "function":
        plan["task"] = {
            "controls": deepcopy(task["controls"]),
            "kind": "function",
            "prompt": task["prompt"],
            "tools": deepcopy(task["tools"]),
        }
    else:
        plan["task"] = {
            "kind": "structured",
            "prompt": task["prompt"],
            "response_format": deepcopy(task["response_format"]),
        }
    return plan
