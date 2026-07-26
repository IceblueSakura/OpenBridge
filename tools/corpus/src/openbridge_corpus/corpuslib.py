from __future__ import annotations

import base64
import hashlib
import json
import random
import re
import shutil
import zipfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft202012Validator, FormatChecker

from . import __version__


DERIVED_DIRECTORIES = {"generated", "reports", "dist"}
STREAM_ARTIFACTS = {"upstream_stream", "expected_client_stream"}
TERMINAL_TYPES = {
    "response.completed": "response_completed",
    "response.failed": "response_failed",
    "response.incomplete": "response_incomplete",
    "error": "error",
}
SECRET_PATTERNS = [
    re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{16,}\b"),
    re.compile(r"(?i)\bauthorization\s*[:=]\s*bearer\s+\S+"),
    re.compile(
        r"(?i)\b(?:api[_-]?key|access[_-]?token)\s*[:=]\s*[\"']?[A-Za-z0-9_-]{16,}"
    ),
    re.compile(r"(?i)\bcookie\s*[:=]\s*\S+"),
]


class CorpusError(RuntimeError):
    pass


@dataclass(frozen=True)
class Case:
    path: Path
    data: dict[str, Any]

    @property
    def directory(self) -> Path:
        return self.path.parent

    @property
    def case_id(self) -> str:
        return str(self.data["id"])


def load_json(path: Path) -> Any:
    try:
        return _loads_json(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise CorpusError(f"{path}: cannot read JSON: {error}") from error


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate object key {key!r}")
        result[key] = value
    return result


def _loads_json(text: str) -> Any:
    return json.loads(text, object_pairs_hook=_reject_duplicate_keys)


def dump_json(data: Any) -> str:
    return json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def _schema_validator(root: Path, name: str) -> Draft202012Validator:
    schema_path = root / "schemas" / f"{name}.schema.json"
    schema = load_json(schema_path)
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, format_checker=FormatChecker())


def _schema_errors(
    validator: Draft202012Validator, data: Any, label: str
) -> list[str]:
    errors: list[str] = []
    for error in sorted(validator.iter_errors(data), key=lambda item: list(item.path)):
        location = ".".join(str(part) for part in error.path) or "(root)"
        errors.append(f"{label}: {location}: {error.message}")
    return errors


def discover_cases(root: Path) -> list[Case]:
    cases_root = root / "cases"
    return [
        Case(path=path, data=load_json(path))
        for path in sorted(cases_root.rglob("case.json"))
    ]


def _resolve_inside(base: Path, relative: str, allowed_root: Path) -> Path:
    candidate = (base / relative).resolve()
    resolved_root = allowed_root.resolve()
    if candidate != resolved_root and resolved_root not in candidate.parents:
        raise CorpusError(f"path escapes allowed root: {relative}")
    return candidate


def _derived_output(root: Path, output: Path | None, directory: str) -> Path:
    allowed_root = (root / directory).resolve()
    candidate = (output if output is not None else allowed_root).resolve()
    if candidate != allowed_root and allowed_root not in candidate.parents:
        raise CorpusError(f"output must stay inside {allowed_root}")
    return candidate


def _parse_sse_events(data: bytes) -> list[dict[str, Any]]:
    text = data.decode("utf-8")
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    blocks = normalized.split("\n\n")
    events: list[dict[str, Any]] = []
    for block in blocks:
        if not block.strip():
            continue
        event_name: str | None = None
        data_lines: list[str] = []
        for line in block.split("\n"):
            if not line or line.startswith(":"):
                continue
            field, separator, value = line.partition(":")
            if separator and value.startswith(" "):
                value = value[1:]
            if field == "event":
                event_name = value
            elif field == "data":
                data_lines.append(value)
        if not data_lines:
            continue
        payload_text = "\n".join(data_lines)
        payload: Any
        if payload_text == "[DONE]":
            payload = "[DONE]"
        else:
            try:
                payload = _loads_json(payload_text)
            except (json.JSONDecodeError, ValueError) as error:
                raise CorpusError(f"invalid SSE data JSON: {error}") from error
        events.append({"event": event_name, "data": payload})
    return events


def terminal_kinds(data: bytes) -> list[str]:
    terminals: list[str] = []
    for item in _parse_sse_events(data):
        payload = item["data"]
        if payload == "[DONE]":
            terminals.append("chat_done")
            continue
        event_type = item["event"]
        if isinstance(payload, dict):
            event_type = payload.get("type", event_type)
        if event_type in TERMINAL_TYPES:
            terminals.append(TERMINAL_TYPES[event_type])
    return terminals


def _validate_case_semantics(case: Case, root: Path) -> list[str]:
    errors: list[str] = []
    data = case.data
    expectation = data["expectation"]
    artifacts = data["artifacts"]
    classification = expectation["classification"]
    stream = data["stream"]

    if case.directory.name != case.case_id:
        errors.append(
            f"{case.path}: directory name must equal case id {case.case_id!r}"
        )

    if classification == "reject":
        if expectation["upstream_attempts"] != 0:
            errors.append(f"{case.path}: reject case must have zero upstream attempts")
        forbidden = {
            "expected_upstream_request",
            "upstream_response",
            "upstream_stream",
            "expected_client_stream",
        }.intersection(artifacts)
        if forbidden:
            errors.append(
                f"{case.path}: reject case has forbidden artifacts {sorted(forbidden)}"
            )
        if "expected_client_response" not in artifacts:
            errors.append(f"{case.path}: reject case requires expected_client_response")
        if data["recipes"]:
            errors.append(f"{case.path}: reject case must not define recipes")
    else:
        if expectation["upstream_attempts"] > 0 and "expected_upstream_request" not in artifacts:
            errors.append(
                f"{case.path}: attempted case requires expected_upstream_request"
            )
        if expectation["upstream_attempts"] == 0:
            forbidden = {
                "expected_upstream_request",
                "upstream_response",
                "upstream_stream",
            }.intersection(artifacts)
            if forbidden:
                errors.append(
                    f"{case.path}: zero-attempt case has upstream artifacts "
                    f"{sorted(forbidden)}"
                )
        if stream:
            if "upstream_stream" not in artifacts:
                errors.append(f"{case.path}: streaming case requires upstream_stream")
            if "expected_client_stream" not in artifacts:
                errors.append(
                    f"{case.path}: streaming case requires expected_client_stream"
                )
            forbidden = {
                "upstream_response",
                "expected_client_response",
            }.intersection(artifacts)
            if forbidden:
                errors.append(
                    f"{case.path}: streaming case has non-stream artifacts "
                    f"{sorted(forbidden)}"
                )
        else:
            if "upstream_response" not in artifacts:
                errors.append(f"{case.path}: non-stream case requires upstream_response")
            if "expected_client_response" not in artifacts:
                errors.append(
                    f"{case.path}: non-stream case requires expected_client_response"
                )
            forbidden = {
                "upstream_stream",
                "expected_client_stream",
            }.intersection(artifacts)
            if forbidden:
                errors.append(
                    f"{case.path}: non-stream case has stream artifacts "
                    f"{sorted(forbidden)}"
                )
            if data["recipes"]:
                errors.append(f"{case.path}: non-stream case must not define recipes")

    terminal = expectation["terminal"]
    terminal_count = expectation["terminal_count"]
    if (terminal == "none") != (terminal_count == 0):
        errors.append(
            f"{case.path}: terminal {terminal!r} and terminal_count "
            f"{terminal_count} are inconsistent"
        )
    if not stream and terminal != "none":
        errors.append(f"{case.path}: non-stream case cannot define an SSE terminal")

    referenced_artifacts: set[Path] = set()
    for artifact_name, relative in artifacts.items():
        try:
            artifact_path = _resolve_inside(case.directory, relative, case.directory)
        except CorpusError as error:
            errors.append(f"{case.path}: {error}")
            continue
        referenced_artifacts.add(artifact_path)
        if not artifact_path.is_file():
            errors.append(f"{case.path}: missing artifact {artifact_name}: {relative}")
            continue
        try:
            raw = artifact_path.read_bytes()
            raw.decode("utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"{artifact_path}: invalid UTF-8 artifact: {error}")
            continue
        if artifact_path.suffix == ".json":
            try:
                _loads_json(raw.decode("utf-8"))
            except (json.JSONDecodeError, ValueError) as error:
                errors.append(f"{artifact_path}: invalid JSON artifact: {error}")
        if artifact_name in STREAM_ARTIFACTS:
            try:
                _parse_sse_events(raw)
            except (CorpusError, UnicodeError) as error:
                errors.append(f"{artifact_path}: {error}")

    expected_stream = artifacts.get("expected_client_stream")
    if expected_stream:
        stream_path = _resolve_inside(case.directory, expected_stream, case.directory)
        if stream_path.is_file():
            try:
                terminals = terminal_kinds(stream_path.read_bytes())
            except (CorpusError, UnicodeError):
                terminals = []
            if len(terminals) != expectation["terminal_count"]:
                errors.append(
                    f"{case.path}: expected terminal_count "
                    f"{expectation['terminal_count']}, observed {len(terminals)}"
                )
            expected_terminal = expectation["terminal"]
            if terminals and any(kind != expected_terminal for kind in terminals):
                errors.append(
                    f"{case.path}: terminal kinds {terminals!r} do not match "
                    f"{expected_terminal!r}"
                )
    elif expectation["terminal_count"] != 0:
        errors.append(
            f"{case.path}: terminal_count is non-zero without expected_client_stream"
        )

    declared_files = referenced_artifacts | {case.path.resolve()}
    for path in sorted(case.directory.rglob("*")):
        if path.is_file() and path.resolve() not in declared_files:
            errors.append(f"{case.path}: undeclared case file {path.relative_to(case.directory)}")

    provenance = data["provenance_ref"]
    try:
        provenance_path = _resolve_inside(root, provenance, root)
        if not provenance_path.is_file():
            errors.append(f"{case.path}: missing provenance {provenance}")
    except CorpusError as error:
        errors.append(f"{case.path}: {error}")

    for recipe in data["recipes"]:
        try:
            recipe_path = _resolve_inside(root, recipe, root)
            if not recipe_path.is_file():
                errors.append(f"{case.path}: missing recipe {recipe}")
        except CorpusError as error:
            errors.append(f"{case.path}: {error}")

    return errors


def lint_corpus(root: Path) -> list[str]:
    root = root.resolve()
    errors: list[str] = []
    required = [
        root / "VERSION",
        root / "catalog.json",
        root / "cases",
        root / "sources",
        root / "recipes",
        root / "schemas" / "catalog.schema.json",
        root / "schemas" / "case.schema.json",
        root / "schemas" / "provenance.schema.json",
        root / "schemas" / "recipe.schema.json",
    ]
    for path in required:
        if not path.exists():
            errors.append(f"missing required corpus file: {path}")
    if errors:
        return errors

    try:
        catalog_validator = _schema_validator(root, "catalog")
        case_validator = _schema_validator(root, "case")
        provenance_validator = _schema_validator(root, "provenance")
        recipe_validator = _schema_validator(root, "recipe")
    except Exception as error:
        return [f"schema initialization failed: {error}"]

    catalog = load_json(root / "catalog.json")
    errors.extend(_schema_errors(catalog_validator, catalog, "catalog.json"))
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    if catalog.get("corpus_version") != version:
        errors.append(
            f"catalog corpus_version {catalog.get('corpus_version')!r} "
            f"does not match VERSION {version!r}"
        )

    try:
        cases = discover_cases(root)
    except CorpusError as error:
        return errors + [str(error)]
    discovered_ids = [case.case_id for case in cases]
    if sorted(catalog.get("case_ids", [])) != sorted(discovered_ids):
        missing = sorted(set(catalog.get("case_ids", [])) - set(discovered_ids))
        extra = sorted(set(discovered_ids) - set(catalog.get("case_ids", [])))
        errors.append(f"catalog/case mismatch: missing={missing}, extra={extra}")

    for case in cases:
        schema_errors = _schema_errors(case_validator, case.data, str(case.path))
        errors.extend(schema_errors)
        if not schema_errors:
            errors.extend(_validate_case_semantics(case, root))

    for path in sorted((root / "sources").glob("*.json")):
        try:
            data = load_json(path)
        except CorpusError as error:
            errors.append(str(error))
        else:
            errors.extend(_schema_errors(provenance_validator, data, str(path)))

    for path in sorted((root / "recipes").glob("*.json")):
        try:
            data = load_json(path)
        except CorpusError as error:
            errors.append(str(error))
        else:
            errors.extend(_schema_errors(recipe_validator, data, str(path)))

    scan_paths: set[Path] = set()
    for case in cases:
        scan_paths.add(case.path)
        for relative in case.data.get("artifacts", {}).values():
            try:
                scan_paths.add(_resolve_inside(case.directory, relative, root))
            except CorpusError:
                pass
    scan_paths.update((root / "sources").glob("*.json"))
    for path in sorted(scan_paths):
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for pattern in SECRET_PATTERNS:
            if pattern.search(text):
                errors.append(f"{path}: suspected secret matched {pattern.pattern!r}")

    return errors


def _chunk_one_byte(data: bytes) -> list[bytes]:
    return [data[index : index + 1] for index in range(len(data))]


def _chunk_lines(data: bytes) -> list[bytes]:
    return data.splitlines(keepends=True) or [data]


def _chunk_utf8_split(data: bytes) -> list[bytes]:
    for index, value in enumerate(data):
        if value >= 0x80 and index + 1 < len(data):
            return [data[: index + 1], data[index + 1 :]]
    midpoint = max(1, len(data) // 2)
    return [data[:midpoint], data[midpoint:]]


def _chunk_seeded(data: bytes, seed: int) -> list[bytes]:
    generator = random.Random(seed)
    chunks: list[bytes] = []
    index = 0
    maximum = max(1, min(17, len(data)))
    while index < len(data):
        size = generator.randint(1, maximum)
        chunks.append(data[index : index + size])
        index += size
    return chunks


def _variant_payload(
    case_id: str,
    artifact: str,
    kind: str,
    seed: int,
    source: bytes,
    chunks: list[bytes],
) -> dict[str, Any]:
    rebuilt = b"".join(chunks)
    if rebuilt != source:
        raise CorpusError(f"{case_id}/{artifact}/{kind}: chunks do not reconstruct source")
    return {
        "artifact": artifact,
        "case_id": case_id,
        "chunks_base64": [
            base64.b64encode(chunk).decode("ascii") for chunk in chunks
        ],
        "encoding": "base64",
        "kind": kind,
        "reconstructed_sha256": sha256_bytes(rebuilt),
        "seed": seed,
        "source_sha256": sha256_bytes(source),
    }


def generate_variants(
    root: Path, seed: int | None = None, output: Path | None = None
) -> dict[str, Any]:
    root = root.resolve()
    errors = lint_corpus(root)
    if errors:
        raise CorpusError("corpus lint failed:\n" + "\n".join(errors))
    catalog = load_json(root / "catalog.json")
    effective_seed = (
        int(seed) if seed is not None else int(catalog["default_generation_seed"])
    )
    output = _derived_output(root, output, "generated")
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)

    generated: list[dict[str, Any]] = []
    for case in discover_cases(root):
        for recipe_ref in case.data["recipes"]:
            recipe = load_json(_resolve_inside(root, recipe_ref, root))
            for artifact_name in sorted(STREAM_ARTIFACTS):
                relative = case.data["artifacts"].get(artifact_name)
                if not relative:
                    continue
                source = _resolve_inside(case.directory, relative, root).read_bytes()
                variants: list[tuple[str, int, list[bytes]]] = []
                for kind in recipe["kinds"]:
                    if kind == "one_byte":
                        variants.append((kind, effective_seed, _chunk_one_byte(source)))
                    elif kind == "line_boundaries":
                        variants.append((kind, effective_seed, _chunk_lines(source)))
                    elif kind == "utf8_split":
                        variants.append((kind, effective_seed, _chunk_utf8_split(source)))
                    elif kind == "seeded":
                        for index in range(recipe["seeded_variants"]):
                            derived = int.from_bytes(
                                hashlib.sha256(
                                    f"{effective_seed}:{case.case_id}:{artifact_name}:{index}".encode()
                                ).digest()[:8],
                                "big",
                            )
                            variants.append(
                                (
                                    f"seeded_{index + 1}",
                                    derived,
                                    _chunk_seeded(source, derived),
                                )
                            )
                for kind, variant_seed, chunks in variants:
                    payload = _variant_payload(
                        case.case_id,
                        artifact_name,
                        kind,
                        variant_seed,
                        source,
                        chunks,
                    )
                    relative_output = (
                        Path(case.case_id)
                        / f"{artifact_name}.{recipe['id']}.{kind}.json"
                    )
                    destination = output / relative_output
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_text(dump_json(payload), encoding="utf-8")
                    generated.append(
                        {
                            "path": relative_output.as_posix(),
                            "sha256": sha256_file(destination),
                        }
                    )

    manifest = {
        "corpus_version": catalog["corpus_version"],
        "files": sorted(generated, key=lambda item: item["path"]),
        "generator_version": __version__,
        "schema_version": "0.1",
        "seed": effective_seed,
    }
    (output / "manifest.json").write_text(dump_json(manifest), encoding="utf-8")
    return manifest


def build_report(root: Path) -> dict[str, Any]:
    root = root.resolve()
    errors = lint_corpus(root)
    if errors:
        raise CorpusError("corpus lint failed:\n" + "\n".join(errors))
    catalog = load_json(root / "catalog.json")
    cases = discover_cases(root)
    status = Counter(case.data["status"] for case in cases)
    classifications = Counter(
        case.data["expectation"]["classification"] for case in cases
    )
    directions: dict[str, Counter[str]] = defaultdict(Counter)
    features = Counter()
    for case in cases:
        directions[case.data["direction"]]["stream" if case.data["stream"] else "non_stream"] += 1
        features.update(case.data["features"])

    pending_sources: list[str] = []
    unpinned_sources: list[str] = []
    for path in sorted((root / "sources").glob("*.json")):
        source = load_json(path)
        if source["license_status"] == "pending":
            pending_sources.append(source["id"])
        if source["ref"] is None:
            unpinned_sources.append(source["id"])

    required = set(catalog["required_core_features"])
    return {
        "case_count": len(cases),
        "classifications": dict(sorted(classifications.items())),
        "corpus_version": catalog["corpus_version"],
        "directions": {
            direction: dict(sorted(counts.items()))
            for direction, counts in sorted(directions.items())
        },
        "feature_counts": dict(sorted(features.items())),
        "missing_required_features": sorted(required - set(features)),
        "pending_license_sources": pending_sources,
        "schema_version": "0.1",
        "statuses": dict(sorted(status.items())),
        "unpinned_sources": unpinned_sources,
    }


def write_report(root: Path, output: Path | None = None) -> dict[str, Any]:
    report = build_report(root)
    if output is not None:
        output = _derived_output(root.resolve(), output, "reports")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(dump_json(report), encoding="utf-8")
    return report


def _packable_files(root: Path) -> Iterable[Path]:
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        if relative.parts and relative.parts[0] in DERIVED_DIRECTORIES:
            continue
        yield path


def pack_corpus(root: Path, output: Path | None = None) -> tuple[Path, str]:
    root = root.resolve()
    errors = lint_corpus(root)
    if errors:
        raise CorpusError("corpus lint failed:\n" + "\n".join(errors))
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    output = _derived_output(
        root,
        output or root / "dist" / f"openbridge-protocol-corpus-{version}.zip",
        "dist",
    )
    output.parent.mkdir(parents=True, exist_ok=True)

    entries = [
        {
            "path": path.relative_to(root).as_posix(),
            "sha256": sha256_file(path),
            "size": path.stat().st_size,
        }
        for path in _packable_files(root)
    ]
    manifest = {
        "corpus_version": version,
        "files": entries,
        "packager_version": __version__,
        "schema_version": "0.1",
    }

    with zipfile.ZipFile(
        output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for path in _packable_files(root):
            relative = path.relative_to(root).as_posix()
            info = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
        manifest_info = zipfile.ZipInfo(
            "manifest.json", date_time=(1980, 1, 1, 0, 0, 0)
        )
        manifest_info.compress_type = zipfile.ZIP_DEFLATED
        manifest_info.external_attr = 0o100644 << 16
        archive.writestr(
            manifest_info,
            dump_json(manifest).encode("utf-8"),
            compress_type=zipfile.ZIP_DEFLATED,
            compresslevel=9,
        )
    digest = sha256_file(output)
    output.with_suffix(output.suffix + ".sha256").write_text(
        f"{digest}  {output.name}\n", encoding="ascii"
    )
    return output, digest
