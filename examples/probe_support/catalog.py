"""Fixed live selections. Never discover credentials, models or endpoints at runtime."""

OAUTH_PROVIDERS = {"codex", "grok"}

BINDINGS = (
    ("codex", "gpt-6.1-sol", "codex-oauth", "codex", ("responses",)),
    ("grok", "grok-4.7", "grok-oauth", "grok", ("responses",)),
    (
        "nvidia",
        "nemotron-3-super",
        "nvidia-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "deepseek",
        "deepseek-flash",
        "deepseek-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "xiaomi",
        "mimo-v2.6-pro",
        "xiaomi-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "xiaomi",
        "mimo-v2.6-flash",
        "xiaomi-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "openrouter",
        "gpt-6-luna",
        "openrouter-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "longcat",
        "longcat-2.5-preview",
        "longcat-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "aliyun-dashscope-cn",
        "qwen3.8-max",
        "aliyun-dashscope-cn-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "aliyun-tokenplan-cn",
        "qwen3.8-flash",
        "aliyun-tokenplan-cn-api-key",
        None,
        ("chat", "responses"),
    ),
    (
        "opencode-go",
        "hy4-preview",
        "opencode-go-api-key",
        None,
        ("chat",),
    ),
    ("zhipu", "glm-5.3", "zhipu-api-key", None, ("chat", "responses")),
    (
        "zhipu",
        "glm-5.3-flash",
        "zhipu-api-key",
        None,
        ("chat", "responses"),
    ),
)


def select_protocol(model, requested=None):
    """Default to declared order; an explicit protocol must be admitted."""
    row = next((row for row in BINDINGS if row[1] == model), None)
    if row is None or requested is not None and requested not in row[4]:
        raise RuntimeError("unknown model or unadmitted protocol")
    return requested if requested is not None else row[4][0]


def select_bindings(selection=None, *, models=None):
    available = dict.fromkeys(row[0] for row in BINDINGS)
    # Subscription-plan usage needs a deliberate selection; a general matrix
    # must not start consuming it just because a new binding was registered.
    # https://help.aliyun.com/en/model-studio/more-tools
    names = (
        selection.split(",")
        if selection is not None
        else [
            name for name in available
            if name not in ("aliyun-tokenplan-cn", "opencode-go", *OAUTH_PROVIDERS)
        ]
    )
    if (
        not names
        or len(set(names)) != len(names)
        or any(name not in available for name in names)
    ):
        raise RuntimeError("unknown, duplicate or paused provider selection")
    rows = [row for name in names for row in BINDINGS if row[0] == name]
    if models is not None:
        admitted = {row[1] for row in rows}
        if (
            not models
            or len(set(models)) != len(models)
            or any(model not in admitted for model in models)
        ):
            raise RuntimeError("unknown, duplicate or out-of-provider model selection")
        rows = [row for row in rows if row[1] in models]
    return rows
