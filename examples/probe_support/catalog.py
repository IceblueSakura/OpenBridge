"""Fixed live selections. Never discover credentials, models or endpoints at runtime."""

BINDINGS = (
    (
        "nvidia",
        "nemotron-3-super",
        "nvidia-primary",
        "OPENBRIDGE_NVIDIA_API_KEY",
        ("chat", "responses"),
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
        "xiaomi",
        "mimo-v2.6-flash",
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
        ("chat", "responses"),
    ),
    (
        "bailian",
        "qwen3.8-max",
        "bailian-primary",
        "OPENBRIDGE_BAILIAN_API_KEY",
        ("chat", "responses"),
    ),
    (
        "aliyun-tokenplan-cn",
        "qwen3.8-flash",
        "aliyun-tokenplan-primary",
        "OPENBRIDGE_ALIYUN_TOKENPLAN_CN_API_KEY",
        ("chat", "responses"),
    ),
    ("zhipu", "glm-5.3", "zhipu-primary", "OPENBRIDGE_ZHIPU_API_KEY", ("chat", "responses")),
    (
        "zhipu",
        "glm-5.3-flash",
        "zhipu-primary",
        "OPENBRIDGE_ZHIPU_API_KEY",
        ("chat", "responses"),
    ),
)


def select_bindings(selection=None, *, models=None):
    available = dict.fromkeys(row[0] for row in BINDINGS)
    # Subscription-plan usage needs a deliberate selection; a general matrix
    # must not start consuming it just because a new binding was registered.
    # https://help.aliyun.com/en/model-studio/more-tools
    names = (
        selection.split(",")
        if selection is not None
        else [name for name in available if name != "aliyun-tokenplan-cn"]
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
