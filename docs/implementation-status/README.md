# 实施现状目录

本页是当前 checkout 实施状态的唯一索引。专题页只记录 live source 已存在的行为、所有权、确定性证据入口、带日期的外部证据
链接和明确未证明范围；功能需求、当前实施授权与外部协议事实分别由 `functional-requirements/`、`implementation-plans/` 和
`references/` 拥有。

同一事实冲突时按“当前 checkout → 对应确定性测试 → 带日期的外部验证记录”处理。外部记录只证明其日期、账号、网络、模型
和 payload，不能覆盖后续 source，也不能替代 SDK、Agent、fallback、负载或长期运行验收。

## 功能状态

| 功能点 | 当前状态页 | 主要确定性证据入口 |
|---|---|---|
| HTTP 网关接口与下游认证 | [HTTP 网关与认证](features/gateway-http-api-and-auth.md) | `tests/ingress_contract.rs`、`tests/downstream_auth_contract.rs`、`tests/mcp_contract.rs` |
| 启动配置、用户与受信凭证 | [启动配置与凭证](features/startup-configuration-and-credentials.md) | `tests/config_contract.rs`、`tests/upstream_credential_config.rs`、`tests/startup_contract.rs` |
| Provider/Model/Target/API/Route/Public Model 注册表 | [注册表与模型目录](features/provider-registry-and-model-catalog.md) | `tests/config_contract.rs`、`tests/provider*_contract.rs`、`tests/forwarding_contract.rs` |
| Models 接口与能力预检 | [Models 与 preflight](features/models-api-and-capability-preflight.md) | `tests/forwarding_contract.rs`、`tests/ingress_contract.rs` |
| Chat/Responses Native 转发 | [Native generation](features/native-generation-forwarding.md) | `tests/forwarding_contract.rs`、`tests/sse_contract.rs` |
| `mimo-v2.5` Native 图片 | [Native 图片](features/native-image-input.md) | `tests/forwarding_contract.rs` |
| Typed Native 文件输入 | [Native 文件](features/native-file-input.md) | `tests/forwarding_contract/file_input.rs`、file profile 单元测试 |
| MiMo 音频理解与专用音频 task | [MiMo 音频](features/native-mimo-audio.md) | `tests/forwarding_contract.rs` |
| Chat ↔ Responses Protocol Bridge | [Protocol Bridge](features/protocol-bridge.md) | `tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs` |
| Retry、fallback、cooldown 与取消 | [韧性与取消](features/resilience-retry-fallback-and-cancellation.md) | `tests/forwarding_contract.rs`、`tests/sse_contract.rs` |
| Embeddings | [Embeddings](features/embeddings.md) | `tests/embedding_forwarding_contract.rs` |
| ChatGPT OAuth2 与 Responses 数据面 | [ChatGPT OAuth2](features/chatgpt-oauth-startup.md) | `tests/oauth2_login_cli.rs`、`tests/startup_contract.rs`、`tests/forwarding_contract.rs` |

## Provider 状态

[Provider 状态目录](providers/README.md)定义真实 Provider、端到端网关与确定性证据的区别。九个编译期 family 均有当前状态页：

| Provider family | 状态页 | 当前证据边界摘要 |
|---|---|---|
| ChatGPT | [chatgpt.md](providers/chatgpt.md) | OAuth/Responses/Bridge 有确定性证据；文字矩阵只走正常首选 source |
| OpenAI | [openai.md](providers/openai.md) | 注册与 Embeddings 有确定性证据；没有成功的真实 Provider 记录 |
| LongCat | [longcat.md](providers/longcat.md) | Native/Bridge 有确定性证据和正常路径文字矩阵 |
| DeepSeek | [deepseek.md](providers/deepseek.md) | Native/Bridge 有确定性证据和定向 structured-output 真实请求 |
| Xiaomi MiMo | [mimo.md](providers/mimo.md) | 文本、图片、音频、tool 的分层证据 |
| OpenRouter | [openrouter.md](providers/openrouter.md) | 固定三 Target；MiniMax/Gemma 有真实证据，未强制 DeepSeek 后备 |
| NVIDIA | [nvidia.md](providers/nvidia.md) | Nemotron Embeddings 有真实证据，MiniMax 后备未强制验收 |
| Alibaba Cloud Model Studio | [bailian.md](providers/bailian.md) | Qwen/GLM/DeepSeek/Embeddings 的分层真实证据 |
| Kimi CN | [kimi-cn.md](providers/kimi-cn.md) | Chat/Responses Bridge 与参数边界有确定性和正常路径证据 |

## 横向状态

| 文档 | 所有内容 |
|---|---|
| [当前代码架构](current-architecture.md) | 稳定模块所有权、装配链和请求数据流；不保存模型/Provider 动态矩阵 |
| [OpenTelemetry 遥测](telemetry-metrics.md) | traces/metrics、OTLP 生命周期、instrument 与安全属性边界 |
| [能力探测](capability-probing.md) | 显式 Target probe 的输入、输出、分类和安全边界 |

## 测试资产与外部证据

| 文档 | 所有内容 |
|---|---|
| [测试资产与保留标准](test-assets/inventory.md) | Rust/Python 测试责任和保留门槛；不保存会漂移的测试总数 |
| [协议语料与工具](test-assets/protocol-corpus.md) | corpus/testkit 版本、case/variant/Python test 数量和验证边界 |
| [带日期的外部验证](evidence/README.md) | 不可变真实 Provider/SDK/Agent 记录；不承担当前状态所有权 |

## 维护规则

1. 新完成行为更新最接近的单一专题；不要再创建第二个“当前实现总览”。
2. 状态页使用“当前行为 → 所有权 → 确定性证据 → 外部证据链接 → 未证明边界”，不追加实施日记或过期计数。
3. 模型、Target、Route 与 Provider 列表只在其当前状态 owner 出现；外部验证按日期新增到 `evidence/`，不改写为“当前”。
4. 测试运行命令和结果必须区分实际执行与未执行；确定性测试不升级为真实 Provider、SDK、Agent、负载或生产验收。
