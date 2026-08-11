# Hermes Agent：model-provider 插件能力与 aux 模型分派

## 范围与证据

- 调研对象：Hermes Agent 本机安装版本 `v0.20.0 (2026.8.3)`，安装目录 `C:\Users\IceblueSakura\AppData\Local\hermes\hermes-agent`。
- 阅读范围：`providers/base.py`、`providers/__init__.py`、`hermes_cli/plugins.py`（PluginManifest/PluginManager kind 语义）、`agent/auxiliary_client.py`（aux 模型分派）、`plugins/model-providers/` 下 deepseek/custom/openrouter/deepinfra/litellm 等示例插件。
- 本文是**外部客户端事实**：Hermes 作为 OpenAI 兼容网关的下游消费者，其 model-provider 插件机制提供哪些能力、aux 模型如何分派。不构成 OpenBridge 的功能承诺。
- 动态事实（字段、解析链、默认值）为 2026-08-08 阅读快照；升级 Hermes 后须重新复核。

关键结论：
1. model-provider 插件的全部能力集中在 **`ProviderProfile` 声明式字段 + 7 个可覆写 hook**，通过 `register_provider()` 注册。
2. **`kind: model-provider` 插件不经过 PluginManager**，不走 `register(ctx)`；想给 provider 插件叠加工具/hook/命令，必须用 `kind: standalone`（见第 6 节）。
3. **aux 模型分派**以 `ProviderProfile.default_aux_model` 为第一来源，fallback 到硬编码 dict，再 fallback 到用户主模型；插件设置 `default_aux_model` 即可完全控制侧任务模型。

## 1. 插件结构与发现机制

### 1.1 目录布局

```
$HERMES_HOME/plugins/model-providers/<name>/
├── plugin.yaml      # 清单（name/kind/version/description/author）
└── __init__.py      # 模块级调用 register_provider(profile)
```

发现顺序（`providers/__init__.py:147 _discover_providers`，懒触发——首次 `get_provider_profile()`/`list_providers()` 调用）：

1. bundled：`<repo>/plugins/model-providers/<name>/`
2. 用户：`$HERMES_HOME/plugins/model-providers/<name>/`
3. legacy：`providers/<name>.py` 单文件（向后兼容）

**覆盖语义**：`register_provider()` 后注册者覆盖先注册者（last-writer-wins，`providers/__init__.py:54`），因此用户插件同名即可替换 bundled 配置——这是第三方定制内置 provider 的官方通道。

### 1.2 plugin.yaml 的 kind 语义（`hermes_cli/plugins.py:284 PluginManifest`）

| kind | 加载方式 | 需要 plugins.enabled？ | 可否 register(ctx) |
|---|---|---|---|
| `model-provider` | 独立 discovery（providers/__init__.py） | 否（天然启用） | **否**——PluginManager 只记录 manifest 供 introspection，不 import（`plugins.py:1440`，避免双实例破坏 last-writer-wins） |
| `standalone` | PluginManager `_load_plugin` | 是 | **是**——可同时 `register_provider()` + `register(ctx)` 注册工具/hook/命令（litellm 插件即此模式） |
| `backend` / `exclusive` / `platform` | 各自类别系统 | bundled 自动 | 视类别 |

litellm 示例（`plugins/model-providers/../litellm/__init__.py`，standalone）：模块级 `register_provider(litellm_provider)` + `def register(ctx)` 注册 8 个 litellm 管理工具。**注意**：plugin.yaml 放在 `plugins/model-providers/` 目录不代表 kind 必须是 model-provider；kind 字段才决定加载路径。

## 2. ProviderProfile 全部字段（`providers/base.py:38`）

### 2.1 声明式字段

| 字段 | 类型 | 默认 | 说明 |
|---|---|---|---|
| `name` | str | — | 规范名（必填）；`/model <name>`、`provider:` 语法用 |
| `api_mode` | str | `chat_completions` | `chat_completions` / `codex_responses` / `anthropic_messages` 等 |
| `aliases` | tuple | `()` | 别名；`get_provider_profile()` 按别名解析 |
| `display_name` | str | `""` | 选择器显示名 |
| `description` | str | `""` | 选择器副标题 |
| `signup_url` | str | `""` | setup 提示 |
| `env_vars` | tuple | `()` | 认证/端点 env key（如 `("MYGW_API_KEY", "MYGW_BASE_URL")`）；缺失时 `hermes plugins install` 交互收集写入 `.env` |
| `base_url` | str | `""` | 推理端点 |
| `models_url` | str | `""` | 显式 models 端点；空则 `{base_url}/models` |
| `auth_type` | str | `api_key` | `api_key` / `oauth_device_code` / `oauth_external` / `copilot` / `aws_sdk` |
| `supports_health_check` | bool | True | False 时 doctor 跳过 /models 探测 |
| `supports_vision` | bool | False | 工具结果消息内是否接受图片 |
| `supports_vision_tool_messages` | bool | True | False 时拒绝 list 型工具消息（Xiaomi MiMo 400 案例） |
| `supports_prompt_cache_key` | bool | False | Chat Completions 是否接受 `prompt_cache_key` 字段（显式 opt-in） |
| `fallback_models` | tuple | `()` | 选择器在 live fetch 失败时的兜底列表；只放支持 tool calling 的模型 |
| `hostname` | str | `""` | URL→provider 反查 hostname；空则从 base_url 推导 |
| `default_headers` | dict | `{}` | 客户端级默认头 |
| `fixed_temperature` | Any | None | None=用调用方默认；`OMIT_TEMPERATURE`=完全不发 |
| `default_max_tokens` | int | None | 用户未显式设置时的输出上限（None=不发，如 Ollama 需要显式避免 128 token 截断） |
| `default_aux_model` | str | `""` | **aux 模型**：压缩/标题/vision/web_extract 等侧任务的廉价模型；空=用主模型 |

### 2.2 可覆写 hooks

| Hook | 签名 | 默认 | 用途/实例 |
|---|---|---|---|
| `get_hostname` | `() -> str` | hostname 字段或 base_url 推导 | URL→provider 反查 |
| `prepare_messages` | `(messages) -> messages` | pass-through | codex 字段清洗后、developer role 交换前的消息预处理 |
| `build_extra_body` | `(*, session_id=None, **context) -> dict` | `{}` | extra_body 字段；OpenRouter 用它发 `session_id`（sticky routing key）、`provider` 偏好、pareto-code `plugins` block（`openrouter/__init__.py:78`） |
| `build_api_kwargs_extras` | `(*, reasoning_config=None, **context) -> (extra_body_additions, top_level_kwargs)` | `({}, {})` | 拆分 extra_body 与顶层 kwargs；DeepSeek 用它发 `extra_body.thinking` + 顶层 `reasoning_effort`（`deepseek/__init__.py`）；custom 用它发 `ollama_num_ctx` → `extra_body.options.num_ctx` 与 `think=false` |
| `default_vision_model` | `() -> str \| None` | None | 运行时发现 vision 默认模型；DeepInfra 从 live catalog 找首个 vision 的 chat 模型（`deepinfra/__init__.py:24`） |
| `get_max_tokens` | `(model) -> int \| None` | 返回 `default_max_tokens` | **按模型变化**的输出上限——聚合网关前端多个输出上限不同的上游时覆写此 hook |
| `fetch_models` | `(*, api_key=None, base_url=None, timeout=8.0) -> list[str] \| None` | GET `{base_url}/models` 取 id 列表 | 定制 auth/path/响应形状；OpenRouter 覆写为无 auth 公共目录 + 模块级缓存（`openrouter/__init__.py:52`） |

## 3. aux 模型分派机制（`agent/auxiliary_client.py`）

### 3.1 解析链（`_resolve_task_provider_model`，`:5795` 附近，源码注释原文）

```
1. 调用方显式 model 参数（调用方知道要什么）
2. Provider 目录默认 —— ProviderProfile.default_aux_model，
   或 legacy _API_KEY_PROVIDER_AUX_MODELS_FALLBACK dict
   （OAuth 类 provider 留空：openai-codex、xai-oauth 的可用模型
   在服务端漂移，不 pin 会腐化的默认值）
3. 用户主模型 model.model（OAuth provider 的 load-bearing 步骤：
   xai-oauth 用户配置 grok-4.3 时标题生成用 grok-4.3）
```

`_get_aux_model_for_provider(provider_id)`（`:700`）实现第 2 步：**先读 `ProviderProfile.default_aux_model`，为空才查 legacy dict**。legacy dict（`:719`）覆盖 gemini/zai/kimi/stepfun/gmi/anthropic/ai-gateway/opencode-zen/kilocode/ollama-cloud/tencent-tokenhub；注释明确"新 provider 应把 default_aux_model 放在 profile 上"。

### 3.2 aux 模型服务的任务（源码注释列举）

- **上下文压缩**（compression）—— 最大头；压缩阈值在 70%~85% 窗口触发（`:581`、`:589`）
- **标题生成**（title generation）
- **视觉任务**（vision）—— 见 3.3
- **提交信息**（commit messages）
- **web_extract**、**session_search**
- **MoA slots**（`moa_reference` / `moa_aggregator`）
- 审批 smart mode（auxiliary LLM 判断）

### 3.3 vision 特例（`:743`、`:753`）

```
_static dict _PROVIDER_VISION_MODELS（xiaomi → mimo-v2.5，zai → glm-5v-turbo）
→ ProviderProfile.default_vision_model() hook
→ 用户主模型
```

静态 dict 优先（xiaomi/zai 的专用视觉模型不在任何可发现目录中）；catalog 驱动型 provider（DeepInfra）用自己的 hook 实现 live 发现。**你的网关若提供专用视觉模型，应在 provider 插件里覆写 `default_vision_model()`**。

### 3.4 MoA 特例（`:2760`）

`provider=moa` 是虚拟 provider：所有 aux 解析层（`_resolve_auto`、`_resolve_task_provider_model`、`resolve_provider_client`）统一经 `_resolve_moa_aggregator` 把 preset 名解析为聚合器的真实 provider+model——preset 名不是合法 wire model id。

### 3.5 aux 解析的统一入口

`resolve_provider_client`（`:5673`）是 client+model 的统一出口；`_resolve_auto`（`:5391`）处理 `provider=auto` 场景（返回与选中 provider 配对的实际模型，不预填 stale 配置）。插件代码若需同步调用 LLM，走 `ctx.llm`（PluginLlm）或 `auxiliary_client` 的公开入口，不要自行拼 client。

## 4. 插件加载与 enablement 语义

- model-provider 插件天然启用（无需 `plugins.enabled`），注册无副作用成本（懒 import，首次 provider 查询才加载）。
- 用户插件同名覆盖 bundled（last-writer-wins）；多个 profile 用唯一模块名隔离（`_hermes_user_provider_<name>`）。
- `requires_env` 缺失时 `hermes plugins list` 标记，但 model-provider 的 discovery 不检查 env——profile 注册照常发生，运行时才缺 key 报错。

## 5. 聚合网关 ProviderProfile 示例

以自研 OpenAI 兼容网关作为 provider 插件接入时：

```python
# $HERMES_HOME/plugins/model-providers/gateway/__init__.py
from providers import register_provider
from providers.base import ProviderProfile

class GatewayProfile(ProviderProfile):
    def get_max_tokens(self, model):      # 上游各有不同输出上限
        return {  # 按上游模型名返回
            "large-model": 8192,
        }.get(model, self.default_max_tokens)

    def build_api_kwargs_extras(self, *, reasoning_config=None, model=None, **ctx):
        # 网关路由参数/上游覆盖/认证头
        extra, top = {}, {}
        if model: top["model"] = model          # 确保 wire model = 网关路由 key
        return extra, top

    def default_vision_model(self):             # 网关有专用视觉模型时
        return "vision-model-id"

gateway = GatewayProfile(
    name="gateway",
    aliases=("gw",),
    display_name="Example Gateway",
    description="自研 OpenAI 兼容聚合网关",
    env_vars=("GATEWAY_API_KEY", "GATEWAY_BASE_URL"),
    base_url="https://gateway.example/v1",
    auth_type="api_key",
    api_mode="chat_completions",
    default_aux_model="fast-model",             # ← 侧任务（压缩/标题/vision/审批）走此模型
    default_max_tokens=65536,                   # 未显式设置时的地板
    fallback_models=("model-a", "model-b"),
)
register_provider(gateway)
```

要点：
- **`default_aux_model` 直接决定网关侧任务模型**——压缩/标题/审批等不占主对话的调用都会打到这个模型，选网关内廉价快速模型。
- `get_max_tokens` 是聚合网关最有价值的 hook：上游输出上限不同时按模型返回。
- `build_api_kwargs_extras` 是 wire 定制唯一入口（extra_body + 顶层 kwargs 双通道）。
- 若还要给网关暴露管理工具（spend/health/models 查询），改用 **`kind: standalone`** 插件：同目录 `plugin.yaml` 去掉 kind 或设为 standalone（经 `plugins.enabled` 启用），`__init__.py` 同时 `register_provider()` + `def register(ctx)`（litellm 模式）。

## 6. 边界与未验证项

- 行号固定于本机 v0.20.0；[Hermes 索引](README.md)记录的 OAuth 专项 checkout（`470cf66b`）与本机安装目录不是同一快照，
  引用行号前以安装目录源码为准。
- 未验证：`kind: model-provider` 插件目录内同时存在 `def register(ctx)` 时 PluginManager 是否完全忽略（源码显示不 import，但未实测）；`auth_type` 各值对聚合网关的影响面（oauth_external/copilot/aws_sdk 分支未逐条核实）；`get_max_tokens` 在 `/model` 切换与 aux 路径是否全量生效。
- 未覆盖：PluginLlm（`ctx.llm`）的完整配置面（`plugins.entries.<id>.llm.*` 门控）；model-provider 插件与 `custom_providers` 配置（`config.yaml`）的关系——插件注册的 profile 是编译期存在，`custom_providers` 是用户运行时覆盖，两者并列。
