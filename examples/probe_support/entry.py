"""Named entry points share execution, lifecycle, reporting and budget ownership."""

import os
from .checks import require, install_signals
from .ledger import Run
from .catalog import select_bindings
from .scenarios import matrix


def entry(kind):
    install_signals()
    gate = {
        "nvidia": "OPENBRIDGE_NVIDIA_PROBE",
        "matrix": "OPENBRIDGE_PROVIDER_MATRIX",
        "gateway": "OPENBRIDGE_GATEWAY_PROBE",
    }[kind]
    require(os.environ.get(gate) == "1", "live_not_enabled", "setup")
    run = Run(os.environ["OPENBRIDGE_PROBE_RUN"])
    os.environ["OPENBRIDGE_PROBE_LIVE"] = "1"
    if kind == "nvidia":
        models = ["nemotron-3-super"]
        case = os.environ.get("OPENBRIDGE_NVIDIA_CASE")
        cases = (case,) if case else ("json", "history", "length", "cancel")
        protocol = "chat"
        delivery = os.environ.get("OPENBRIDGE_NVIDIA_DELIVERY")
        effort = os.environ.get("OPENBRIDGE_NVIDIA_EFFORT")
    elif kind == "gateway":
        models = ["gpt-6-luna"]
        protocol = delivery = effort = None
        cases = (
            ("reasoning",)
            if os.environ.get("OPENBRIDGE_GATEWAY_REASONING") == "1"
            else ("text", "tool")
        )
    else:
        selected = os.environ.get("OPENBRIDGE_MATRIX_PROVIDERS")
        models = (
            [row[1] for row in select_bindings(selected)]
            if selected is not None
            else run.plan["models"]
        )
        case = os.environ.get("OPENBRIDGE_MATRIX_CASE")
        cases = (case,) if case else ("text", "tool")
        protocol = os.environ.get("OPENBRIDGE_MATRIX_PROTOCOL")
        delivery = os.environ.get("OPENBRIDGE_MATRIX_DELIVERY")
        effort = None
    require(
        protocol in (None, "chat", "responses") and delivery in (None, "json", "sse"),
        "filter",
        "setup",
    )
    require(all(model in run.plan["models"] for model in models), "selection", "budget")
    return (
        0
        if matrix(
            run,
            models,
            cases=cases,
            protocol=protocol,
            delivery=delivery,
            effort=effort,
        )
        else 1
    )
