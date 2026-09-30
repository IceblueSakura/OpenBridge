"""Fixed live selections. Never discover credentials, models or endpoints at runtime."""

BINDINGS = (
    (
        "nvidia",
        "nemotron-3-super",
        "nvidia-primary",
        "OPENBRIDGE_NVIDIA_API_KEY",
        ("chat",),
    ),
    (
        "deepseek",
        "deepseek-flash",
        "deepseek-primary",
        "OPENBRIDGE_DEEPSEEK_API_KEY",
        ("chat", "responses"),
    ),
    (
        "xiaomi",
        "mimo-v2.6-pro",
        "mimo-primary",
        "OPENBRIDGE_XIAOMI_API_KEY",
        ("chat", "responses"),
    ),
    (
        "openrouter",
        "gpt-6-luna",
        "openrouter-primary",
        "OPENBRIDGE_OPENROUTER_API_KEY",
        ("chat", "responses"),
    ),
    (
        "longcat",
        "longcat-2.5-preview",
        "longcat-primary",
        "OPENBRIDGE_LONGCAT_API_KEY",
        ("chat",),
    ),
    (
        "bailian",
        "qwen3.8-max",
        "bailian-primary",
        "OPENBRIDGE_BAILIAN_API_KEY",
        ("chat",),
    ),
    ("zhipu", "glm-5.3", "zhipu-primary", "OPENBRIDGE_ZHIPU_API_KEY", ("chat",)),
)


def select_bindings(selection=None):
    names = (
        selection.split(",") if selection is not None else [row[0] for row in BINDINGS]
    )
    available = {row[0]: row for row in BINDINGS}
    if (
        not names
        or len(set(names)) != len(names)
        or any(name not in available for name in names)
    ):
        raise RuntimeError("unknown, duplicate or paused provider selection")
    return [available[name] for name in names]
