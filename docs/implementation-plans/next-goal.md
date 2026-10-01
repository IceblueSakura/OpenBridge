# 下一步目标

**下一步设计重点是 Agent-first、协议中立的 Generation 交互与依赖合同，而不是先追齐某家 API 的字段。** 项目尚未上线，允许在明确设计与获准实现切片中替换现有类型；一套 IR 指共享原则和 task family，不是万能请求。产品方向见 [v2 目标](../architecture-v2/README.md)，详细设计只由[semantic IR](../architecture-v2/semantic-ir.md)维护；实现缺口归[当前能力与边界](../implementation-status/generation.md)。不以迁移、追平或恢复旧版为目标。

**既有 Codex / SuperGrok OAuth2 登录方向保留：先进一步调研授权合同与 credential 生命周期，再分别形成实现切片。** 它是独立的 credential/execution 工作，不以新 IR 的全部落地为前置，也不替代下述设计顺序。Codex 的旧资料可作技术定位；SuperGrok 的实际认证机制及第三方接入资格仍待核实，目标名称不构成已支持 OAuth2 的声明。来源与采用边界见 [OAuth 登录来源](../references/oauth-login.md)。

最小 HTTP 入口的职责和运行边界见 [HTTP 指南](../http-gateway.md)。当前 Provider/模型与协议准入按 [AGENTS.md](../../AGENTS.md#current-provider-model-and-compatibility-information)现场查询；本页不维护支持清单、实测结果或临时账号阻塞。

## IR 设计优先顺序

以下是设计产物与定稿门槛，不是自动获准的代码任务。跨模块合同在现有 semantic IR/ADR owner 内维护，不新增协议比较报告；具体 wire 字段与测试场景留给获准实现切片。

| 优先级 | 设计主题 | 定稿产物与边界 |
|---|---|---|
| P0 | 交互、结果与控制转移 | 明确 item/message/response/turn/group/call/result；响应闭合、产物完整性与 turn 进度的合法组合；continuation 要求与实际调度、重试的边界 |
| P0 | Identity、分组与 replay 依赖 | 明确稳定 identity、wire 坐标、单 owner 与组/前缀依赖；opaque 类型的 scope/finality、编辑失效及客户端交付→回传合同 |
| P1 | 内容、工具结果与资源 | 明确媒体值和用途、结构化结果及工具错误、资源来源/权限边界、引用的输出 owner 与来源坐标；不建任意嵌套可执行容器 |
| P1 | 控制、Schema、cache 与 usage | 区分模式/预算/显示、声明约束/目标保证、亲和提示/前缀策略/资源引用、报告计数/有前提的派生视图；避免同名即等价 |
| P2 | 上下文演进 | 定义配置变化、compaction、远端 continuation 资源的作用范围与依赖；不据此引入 session 服务或自动 Agent loop |

### 定稿方法与下一片选择

1. 先定义概念、唯一 owner、presence、合法状态及关系，不先承诺 Rust struct 或兼容 alias。
2. 用 OpenAI、Google、Anthropic 的独立官方合同检查请求/history、响应与事件；记录不可表示及有条件映射，不以协议名称或类型存在推定能力。来源入口见[references](../references/README.md)。
3. 对插入、替换、删除、重排明确依赖失效和目标拒绝；评审方法归[验收基线](../references/conformance-baseline.md)。
4. 在相应能力内选定具体协议/API 版本、下游 carrier、scope 构造、资源边界及是否允许损失转换。未知事项不以万能 metadata/JSON 字段填补。
5. 只有选定端到端行为后，才在[current-focus](current-focus.md)写入获准实现 slice；同步类型、codec/lowering、序列化与适用公共合同。设计文档不扩大现有 Chat/Responses profile，不证明 SDK、上游或生产可用。

优先选择能检验工具续轮、响应/turn 分离与 replay 依赖的最小场景，再扩展媒体宽度；无需先实现所有 Provider、所有任务或通用工作流引擎。后续每片应列明已定稿合同和仍待选择的边界，不能以本设计授权真实调用或凭据操作。

## 保留的实施方向

下表保留既有工作顺序，不表示这些代码任务已获准或已实现。IR 相关行为变化需先完成上面的对应设计，而非等待全部设计域结束。

| 优先级 | 建议切片 | 退出条件 |
|---|---|---|
| 1 | 调研后实现 Codex / SuperGrok OAuth2 登录 | 先重新固定合法登录、refresh、账户绑定与订阅推理合同，分别定稿 owner 和安全边界；再按获准切片实现并以独立 synthetic 登录/刷新/失败用例验证。真实登录与 Provider 调用另行授权，不由方向文档触发 |
| 2 | 按实际需求选择端到端文本场景 | 现场核对绑定、客户端和授权范围；请求经过 binary/Router，覆盖 IR、交付、工具续轮与失败边界，不仅是库级 decode 或 HTTP 200 |
| 3 | 选定 Agent/Provider 原生缓存场景与必要文本投影 | 核对稳定前缀、Schema/工具顺序、replay scope 与派生 view；按消费需求补投影。评价缓存效果时独立设计对照，不把兼容默认值当计费事实 |
| 4 | 与实际使用相称的运行保障 | 按具体需求决定凭据生命周期、诊断、负载与失败策略；未选定前不预建动态 registry、通用插件或完整旧运行时 |
| 5 | 扩展 Provider 与多模态 | 以现有 user URL/inline 图片输入为起点，按需求选定 file_id 来源/生命周期、工具媒体结果、其他模态或独立媒体任务；新 wire 差异改 adapter，真正的新能力演进共享 task/extension owner，同时验收 request/response/event 与资源边界 |

不以重复成功矩阵代替问题定位，也不根据过期结果固定下一轮目标。新发现先区分上游输出、字段投影、I/O 生命周期和客户端差异；稳定结论进入 owning code 注释与独立 synthetic 回归，运行结果只在当次交付和授权 run 中保留。

可读 reasoning 与 opaque continuation 分别按实际合同验收：可读内容检查归属、历史和变换保真，不能替代所需 signature 的回放验证；opaque 值需要明确格式、owner、origin、依赖与 finality。无 opaque 合同的场景不强造密文；有回放要求时不能只保留可读内容。不为尚未选定的模型预建通用透传，不静默修改请求控制来通过测试。

现有缓存投影从 Provider 自动缓存与明确的 cache key/session carrier 起步；新设计中的前缀策略和资源引用需另行定稿与准入。不在本项目实现负载均衡、回答缓存、会话管理或跨请求粘性路由。稳定公开接入与扩展 owner 见 [ADR 0011](../architecture-v2/decisions/0011-stable-admission-provider-cache.md)。

## OAuth 登录调研与实现前置条件

1. **分别确认目标合同**：Codex 指 ChatGPT 账户登录及其订阅 backend，不是 OpenAI API-key 登录；SuperGrok 不等于 OpenRouter 的 Grok 模型或 xAI API-key 接入。先确认 authority、合法 client registration、scope/audience、callback/device flow、账号/workspace 绑定及自动化使用资格；不把 Codex 私有流程推广为通用 OAuth adapter。
2. **确定生命周期与 owner**：明确显式登录/取消/重新授权、secret storage、access expiry、refresh rotation/revocation、并发 single-flight、持久化一致性和不确定结果处理。认证 owner 与 Provider 执行边界协作，但 token、locator、选定账户、refresh/retry state 不进入 Task IR，纯 codec/lowering 不访问 credential 或网络。
3. **保持受信路由与失败边界**：业务请求只能提交 public model，不能指定 Provider、账户、authority 或凭据。区分共享 canonical 身份与具体 credential/replay scope；401 不自动授权跨账户切换或无界重试，publication/commit 后不重放。普通请求不能隐式发起交互登录；不搜索或导入 Codex、Hermes、LiteLLM 或浏览器的认证缓存。
4. **先离线、后明确授权验收**：使用 synthetic authority/存储验证成功、拒绝、过期、取消、rotation、并发和写入失败。采用结果进入受影响的 ADR、owning code 和必要操作指南，不恢复旧报告或整个旧运行时。真实登录、token refresh、订阅推理和凭据写入须另行明确目标、效果与输出边界；若还需付费调用，另定请求和 token 预算。

本页只确定方向，不授予 OAuth 实现、登录或凭据操作权限。具体实现范围以 [current-focus](current-focus.md) 维护的获准切片为准；调研充分后在该 owner 记录可观察行为、失败用例、非目标与验证边界，再按获准范围推进。

## 下一片需选定的边界

- 从当前源码和启动入口确认模型、客户端及同协议或可表示的跨协议子集；实际实例启用与账号可用性分开判断。
- 若需真实调用，先约定目标、精确请求矩阵、token/请求数与脱敏边界；方向文档、旧计划和存在密钥都不是授权。
- 检查客户端 → 入口认证 → IR → Provider → 实际下游 body → 下一轮回放的整条路径。
- 保持固定可信目标、预算、late failure 不伪造终态、交付后不 retry/fallback 的边界；不靠延长 deadline 或宽松解析掩盖失败。
- 在 [current-focus](current-focus.md)记录获准行为切片，以独立反例驱动实现；完成后恢复为空，不留下测试日记。

无需把 hosted tools、program 执行、Codex turn 管理、state/WS 或其他 task family 的完整实现当作每个切片的前置条件；也不因本机入口存在而宣称生产就绪。
