# Hermes Agent：聚合网关可用的插件化扩展面

## 范围与证据

- 调研对象：Hermes Agent 本机安装版本 `v0.20.0 (2026.8.3)`，安装目录 `C:\Users\IceblueSakura\AppData\Local\hermes\hermes-agent`。
- 阅读范围：`hermes_cli/plugins.py`（PluginContext/PluginManager、VALID_HOOKS、VALID_MIDDLEWARE 引用）、`hermes_cli/middleware.py`（middleware 契约）、`agent/auxiliary_client.py`（aux 分派）、`plugins/model-providers/litellm`（standalone 管理工具先例）、`plugins/web/`、`plugins/platforms/`。
- 前置文档：`hermes-provider-plugin-capabilities.md`（ProviderProfile 字段/hooks 与 aux 分派）。模型 metadata 不在本地 reference 中复制。
- 本文评估对象：作为 Hermes 推理后端和工具提供方的自研 OpenAI-compatible 聚合网关；只比较插件系统提供的外部接入面。
- **未覆盖**：`register_secret_source`；源码语义见 plugins.py:824，插件 source 不参与首进程 bootstrap。
- 动态事实为 2026-08-08 阅读快照；升级 Hermes 后须重新复核。

## 1. 框架：网关在 Hermes 面前的三个角色

| 角色 | 插件面 | 价值排序 |
|---|---|---|
| **推理后端**（LLM 调用方） | ProviderProfile（前一篇已覆盖）+ `llm_request` middleware + 观测 hooks | ★★★ |
| **工具提供方**（agent 可调用的工具） | `register_tool`（管理工具）、`register_web_search_provider`、MCP 服务、`register_browser_provider` | ★★★ |
| **服务方**（生命周期/策略/密钥/平台） | hooks、middleware、`register_auxiliary_task`、`register_platform` 等 | ★★ |

以下按价值分层列出每项能力、源码位置、精确语义与网关场景。

## 2. 高价值扩展点

### 2.1 `llm_request` middleware：请求时动态路由（网关核心价值）

- API：`ctx.register_middleware("llm_request", cb)`；回调签名 `(request, **context) -> {"request": <新 api_kwargs>}`。
- 时机（`agent/conversation_loop.py:2240`）：**每次**发给 provider 的请求发出前；多个 middleware 按注册顺序链式执行，前一个输出是后一个输入。
- 可改写：`request` 就是传给 provider client 的同一 `api_kwargs`——`messages`、`system`、`model`、`max_tokens`、`stream`、`extra_headers`、`extra_body` 全部可改。
- context 参数：`session_id`、`turn_id`、`api_request_id`、`task_id`、`platform`、`model`、`provider`、`base_url`、`api_mode`、`api_call_count`。
- 与 ProviderProfile hook 的分工：`build_extra_body`/`build_api_kwargs_extras` 是**静态/配置级** wire 定制；`llm_request` middleware 是**动态/会话级**。二者叠加构成完整请求定制面。
- 网关场景：
  - 按 `session_id` 固定上游（会话粘性），按 `api_call_count`/`api_request_id` 做重试换路
  - 按 `platform`/`task_id` 注入不同路由参数（`extra_headers` 带上游偏好、`extra_body` 带路由 key）
  - 按 `model` 兜底改写（网关前端模型名 → 上游真实模型）
  - 成本/配额门控（会话级预算内才放行）

### 2.2 网关管理工具集（`kind: standalone` + `register_tool`）

- 先例：`plugins/model-providers/litellm/` 是完整模板——模块级 `register_provider()` + `def register(ctx)` 注册 8 个管理工具（list_models/model_info/search/spend/key/sync/health）。
- 关键陷阱：**`kind: model-provider` 插件不走 PluginManager、不执行 `register(ctx)`**（`hermes_cli/plugins.py:1440`）。要叠加工具/hook/命令，plugin.yaml 的 kind 必须是 `standalone`（或省略），经 `plugins.enabled` 启用。
- `register_tool` 签名：`(name, toolset, schema, handler, check_fn=None, requires_env=None, is_async=False, description="", emoji="", override=False)`；handler `(args, **kwargs) -> JSON str`。
- 网关场景（litellm 同构）：
  - `gw_health` —— 上游连通性/延迟/负载
  - `gw_models` —— 模型列表与能力（配合 /models 动态 context 上报）
  - `gw_spend` / `gw_route_status` —— 成本与路由状态
  - `gw_set_route` —— 运行时切换上游（配合 2.1 middleware 实现换路）
  - 若工具名与内置冲突，`override=True` 需 `plugins.entries.<id>.allow_tool_override: true` 信任门（bundled 插件默认信任，第三方默认拒绝，fail-closed）。

### 2.3 `register_web_search_provider`：无前缀替换内置搜索

- API：`ctx.register_web_search_provider(WebSearchProvider)`；`provider.name` 匹配 `web.search_backend` / `web.extract_backend` / `web.backend` 配置，路由 `web_search` / `web_extract` 工具调用（`plugins.py:764`）。
- 语义对比：MCP 暴露的工具名固定带 `mcp_<server>_` 前缀；**想让 agent 无感使用网关的搜索实现（工具名就叫 `web_search`）必须走 web search provider 插件**，MCP 只适合暴露 Hermes 没有的独特工具。
- 网关场景：网关 MCP 服务里有 web_search 时，二选一——保留前缀（MCP 直接接）或注册 provider（无前缀，替代内置 brave/tavily 等）。

### 2.4 观测 hooks：Hermes 视角的上游运行数据

- `post_api_request`：`api_duration`、`finish_reason`、`message_count`、`response_model`、`usage`、`assistant_content_chars`、`assistant_tool_call_count`（示例见 `hermes_cli/hooks.py:198`）。
- `api_request_error`：失败请求的模型/端点/错误分类。
- 与网关自观的区别：Hermes 侧能看到 retry 次数、model 切换、tool loop 结构、压缩触发点——补齐调用链两端视角。
- 网关场景：把上述字段转发到网关监控/计费/归因；`post_tool_call`/`transform_tool_result` 还可把 Hermes 本地工具执行（terminal/file/web）的结果摘要回传网关审计。

### 2.5 `register_auxiliary_task`：网关专属 LLM 侧任务

- API：`ctx.register_auxiliary_task(key, *, display_name, description, defaults)`（`plugins.py:1069`）。
- 效果：任务自动出现在 `hermes model → Configure auxiliary models` 选择器；有独立 `auxiliary.<key>` 配置块；gateway 启动时桥接 `AUXILIARY_<KEY_UPPER>_*` env；默认路由字段（provider/model/base_url/api_key/timeout/extra_body）自动合并。
- 约束：key 为 snake_case，不能 shadow 内置任务（vision、compression、web_extract、approval、mcp、title_generation、skills_hub、curator）；同名跨插件冲突被拒。
- 网关场景：
  - `gw_route_health` —— 周期性评估各上游健康并产出报告
  - `gw_cost_report` —— 成本归因汇总（走网关而非自带 key）
  - 注意：这是"声明一个 aux 任务配置入口"，实际调用仍需插件自己的工具/hook 触发；内置 aux 任务（压缩/标题/vision/审批）走 `default_aux_model`（前一篇 3.1）。

## 3. 中价值扩展点

| 能力 | API / hook | 精确语义（源码） | 网关场景 |
|---|---|---|---|
| 工具调用审计/限流 | `pre_tool_call` / `post_tool_call` hooks | 只读观察，返回值被忽略（`plugins.py:135` 起 VALID_HOOKS 注释） | Hermes 本地工具（terminal/file/web）的成本、限流、结果摘要记到网关 |
| 工具参数改写 | `tool_request` middleware（`middleware.py:120`） | 可返回 `{"args": {...}}` 替换参数，早于 hooks/guardrails/approvals | 网关策略下发：对特定工具的调用注入参数/降级 |
| 工具执行包装 | `tool_execution` middleware（`middleware.py:206`） | 可包裹真实回调（before/after/改写结果） | 网关侧缓存工具结果、幂等去重 |
| 响应文本改写 | `transform_llm_output` hook | 插件返回 string 替换返回给用户的文本，首个非 None 生效 | 剥离 reasoning_content、统一格式、标注上游 |
| 工具结果改写 | `transform_tool_result` / `transform_terminal_output` | 同 transform 系列 | 网关特有格式清洗 |
| 会话生命周期 | `on_session_start` / `on_session_end` / `on_session_reset` | 会话级事件 | 上游分配、网关会话创建/回收 |
| 每 turn 上下文注入 | `pre_llm_call` hook | 唯一可返回 `{"context": ...}` 注入到当前 turn user message（默认 10k 字符上限 + spill 落盘） | 注入网关状态/当前路由/成本提示 |
| 验证门控 | `pre_verify` hook | 返回 `{"action": "continue", "message": ...}` 阻止 agent 收尾（受 `agent.max_verify_nudges` 限制） | 配合网关做"验证前检查"策略 |
| 浏览器后端 | `register_browser_provider` | `provider.name` 匹配 `browser.backend`，路由 `browser_navigate` 等 | 网关若也代理 browser API |
| 插件内推理走网关 | `ctx.llm`（PluginLlm） | 用用户活动模型/auth 跑 chat/结构化补全；override 能力 fail-closed，`plugins.entries.<id>.llm.*` 门控 | 网关管理工具内部的 LLM 分析不另带 key |
| 调用任意工具 | `ctx.dispatch_tool(name, args)` | 以父 agent 上下文调用，走完整审批/脱敏管线 | 管理工具内复用 `terminal`/`web_search` 等 |
| 消息注入 | `ctx.inject_message(content)` | 向活动会话注入，idle 时启动新 turn | 网关事件（如上游故障）主动通知 agent |

## 4. 低价值 / 不建议（对 headless 聚合网关）

| 能力 | 源码 | 为什么不建议 |
|---|---|---|
| `register_platform`（gateway 频道适配器） | `plugins.py:953`，`adapter_factory(cfg) -> BasePlatformAdapter` | 把网关做成 messaging platform 是反向耦合；headless 网关不适合。唯一例外是 bundled 的 **A2A** 平台插件（agent 互操作协议），但那属于产品决策而非插件功能 |
| `register_context_engine` | `plugins.py:638`，替换内置 ContextCompressor，单例 | 除非网关提供压缩服务，否则不值得 |
| `register_dashboard_auth_provider` | `plugins.py:697`，web dashboard OIDC/auth | 网关带独立 dashboard 时才相关 |
| `register_slack_action_handler`、`register_tts_provider`、`register_transcription_provider`、`register_cron_provider`、`register_image_gen_provider`、`register_video_gen_provider` | 对应分类注册器 | 与推理聚合网关的核心定位无关；除非网关同时代理这些 API |

## 5. wire 定制通道汇总（网关最常用）

| 通道 | 层级 | 时机 | 改什么 | 网关用途 |
|---|---|---|---|---|
| `ProviderProfile.build_extra_body` | 静态 | 每次请求 | extra_body 字段 | 固定路由参数（session_id sticky key 等） |
| `ProviderProfile.build_api_kwargs_extras` | 静态 | 每次请求 | extra_body 追加 + 顶层 kwargs | reasoning/thinking 适配、模型名路由、固定头 |
| `ProviderProfile.get_max_tokens(model)` | 静态按模型 | 输出上限 | 上游各有不同输出上限时按模型返回 |
| `llm_request` middleware | **动态会话级** | 每次请求发出前 | 整个 api_kwargs | 会话粘性、重试换路、按平台注入、配额门控 |
| `pre_llm_call` hook | 观察+注入 | 每次 LLM 调用 | 追加 context 到 user message | 注入网关状态/路由提示 |

## 6. 能力依赖分层（研究推论）

```
基础协议：ProviderProfile + default_aux_model + /models dynamic context
动态请求：llm_request middleware
管理能力：kind: standalone + register_tool
工具提供：MCP；需要无前缀时使用 register_web_search_provider
观测能力：post_api_request/api_request_error + register_auxiliary_task
生命周期：on_session_*、pre_verify、transform_* 等 hooks
```

前两层决定网关作为推理后端的请求合同；管理和工具层提供额外 surface；观测与生命周期 hooks 属于独立运维能力。该分层只描述
Hermes 插件面的依赖关系，不是任何具体产品的实施顺序。

## 7. 边界与未验证项

- 行号固定于本机 v0.20.0；[Hermes 索引](README.md)记录的 OAuth 专项 checkout（`470cf66b`）与本机安装目录不是同一快照，
  引用行号前以安装目录源码为准。
- 未验证：`llm_request` middleware 在 aux 调用路径（compression/title/vision）是否同样生效（源码只确认主 conversation_loop 调用点；aux 路径经 `auxiliary_client` 独立构造请求，未逐行核实是否经过 middleware 链）；`register_auxiliary_task` 的 env 桥接与 picker 集成未实测；`transform_tool_result` 的具体返回契约（替换还是包装）未逐行确认；A2A 平台插件的实际能力面未调研。
- 未覆盖：PluginLlm 的完整配置面；`register_platform` 各 adapter 实现细节；middleware 与 hooks 在 gateway/kanban worker 进程的执行差异。
