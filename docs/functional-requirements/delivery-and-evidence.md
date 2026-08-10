# TDD 开发方式与证据记录

## 状态

**现行工作方式。** OpenBridge 不预先固化开发阶段、阶段依赖、退出条件或全局验收标准。开发以一个个可观察行为推进；每个行为的测试是该行为的局部证据，不是产品整体完成声明。

## 工作循环

对每个当前焦点，按以下循环工作：

1. 从[产品范围](product-scope.md)或一个已知缺陷中选择一个可观察行为，写清楚输入、预期输出和不覆盖的边界。
2. 先添加或调整一个会失败的自动化测试、fixture 或最小客户端复现。
3. 只实现足以让该测试通过的最小代码；不为尚未有测试的未来方向预建抽象。
4. 运行与该行为相称的回归：先运行本地单元/契约测试，再使用 OpenAI SDK、独立 Python 脚本或 curl
   复核实际客户端可见行为；只有明确的目标客户端兼容行为才使用对应客户端 runtime。
5. 在测试保持通过的前提下重构；若发现新语义，先补失败测试而不是扩展原实现。
6. 更新[当前实现总览](../implementation-status/current-implementation.md)链接的功能专题中的已证明事实。当前开发焦点完成后替换或清空，不在原文档累积下一批工作包。

如果某项工作会改变产品目标、信任边界或非目标，先修改基础目标并说明取舍；否则不需要先设计新的 phase 或验收门。

## 预发布破坏性变更原则

OpenBridge 尚未发布任何版本，也没有承诺支持的外部部署、SDK profile、配置 schema 或持久化数据格式。当前焦点内允许大范围删除、
替换或重组原型代码，包括内部与 public crate API、bootstrap/DTO shape、模块所有权、fixture 和测试；决策目标是形成首个一致、可验证的
产品契约，而不是维持历史原型形状。

- 不为未发布行为增加 legacy alias、双读写/双实现、自动猜测迁移、弃用窗口或只为表达变更而递增的 schema/version。直接修正当前
  契约，并同步 parser、serialization、OpenAPI、示例、fixture、文档和测试。
- “最佳实践”以所用 Rust/framework 的明确所有权与生命周期、结构化错误、取消传播、异步失败隔离、有界资源、安全默认值、最小
  credential 暴露、可替换测试边界和必要依赖为判断依据；不以文件行数、流行模式或未被当前测试需要的抽象层为依据。
- 大范围重构必须服务于当前唯一可观察行为，并由先失败测试保护；预发布状态不授权并行实现第二个功能、顺手扩展 Provider、引入动态
  控制面或降低验收层级。
- 已存在的私有配置、credential 和用户数据不视为可随意销毁。若配置契约被直接修正，示例和错误必须同步，使旧形状安全、明确地
  fail closed；不得读取、重写或提交用户的私有文件来完成迁移。
- 完成时不保留“新旧路径都能工作”的临时状态；删除被替代代码与冗余测试，并用当前需求、实现现状和实际验证结果描述唯一受支持形状。

## 验证优先级

日常验证按以下优先级选择，按行为需要叠加，而不是要求每次都跑完整大套件：

| 层次                        | 主要用途                                               | 说明                                                                              |
|-----------------------------|--------------------------------------------------------|-----------------------------------------------------------------------------------|
| Rust 单元/集成/fixture 测试 | 快速、确定地保护单个行为                               | 应覆盖成功、预期错误、流边界和取消等与当前改动相关的路径。                        |
| OpenAI SDK                  | 验证 Chat 与 Responses 的客户端可见 HTTP/SSE 行为      | 是首选日常互操作证据；同时覆盖 stream/non-stream，按改动需要覆盖 tool loop。      |
| 独立 Python 脚本或 curl     | 以最小客户端复现验证 HTTP header、JSON、SSE 与错误语义 | 是首选日常协议证据；应保持无 Agent runtime 依赖并可脱敏重跑。                     |
| 目标 Agent 客户端           | 验证该客户端特有的 profile、扩展或真实 tool loop       | 仅在明确声明 Codex、Hermes 等具体客户端兼容时使用，不作为通用行为的默认验收依赖。 |
| 真实 Provider               | 定位 Provider、模型、配额或网络特有差异                | 只在 SDK/独立客户端/fixture 无法解释行为，或当前改动直接涉及该 Provider 时使用。  |

离线 fixture 用于可重复的 framing、错误、EOF、partial stream、cancel 与 tool-call 回归；它们不替代目标客户端观察。真实
Provider 一次成功也不替代可重复测试。

### 新 endpoint 的 fake-first 合同证据

新增 OpenAI-compatible endpoint 时，可以先在尚未选择真实 Provider/model 的情况下完成协议合同：测试专用的 synthetic
Provider/Target/Route/Public Model、loopback upstream、resource store 或 session simulator 可以作为先失败测试的执行边界。它们必须走与
真实请求相同的下游 router、认证、body limit、request analysis、planning、transport、renderer、错误与终态观测路径；只直接调用内部
serializer 或返回固定 `200` 不足以证明 endpoint 客户端可见行为。

fake 资产必须同时满足：

- 仅存在于 test/fixture 组装，不进入 production bootstrap、示例配置或 production `/v1/models`；
- 使用合成 id、内容和 credential，不读取真实 Provider 配置，不发起真实网络调用；
- JSON operation 覆盖成功、字段拒绝、上游错误和取消；multipart/binary/SSE 进一步覆盖 content type、分片、budget、terminal 与首个
  下游 byte 后失败；
- resource、async job 或 session 不以固定 canned body 冒充，分别覆盖 issuer affinity、分页/过期/删除，合法状态转换/取消/幂等，或
  双向 event/close/backpressure；
- 可用官方 SDK 或独立 curl/Python 对 loopback test server 做协议观察，但记录为 fake/loopback 证据。

generic endpoint contract 通过后，可以再选择满足该 operation 的真实 Provider/model，并单独验证官方 Provider path、认证、能力、limit、
真实成功/错误和适用 transport。若 operation 由 model 选择，接入还须完整注册 canonical model、Target、Route、Public Model 与 Models
projection；若 operation 围绕资源执行，则必须固定 issuer 与 account/region affinity。只有 fake 通过时不得向 production 下游公开
synthetic model，也不得声称任何真实 Provider、model、media quality、retention、费用或负载已经可用。

## SDK 与客户端工具的滚动记录

不对 OpenAI SDK、Python/curl 工具或目标客户端设长期固定版本。每次需要作为证据保存的运行，应记录：

- 实际解析的 SDK、Python/curl 或目标客户端版本与安装来源；
- 操作系统、架构和相关运行时版本；
- 无密钥的最小配置、endpoint/transport 选择和重跑命令；
- 使用 mock、fixture、SDK、独立 Python/curl、目标 Agent 客户端（如适用）还是真实 Provider；
- 脱敏后的输入、关键观察、错误分类以及“该运行证明什么/不证明什么”。

仓库中出现的固定版本号只能表示某次历史实验或 fixture 的实际环境，不能被解释为后续开发的环境锁定要求。

## 当前焦点的最小记录

[当前开发焦点](../implementation-plans/current-focus.md)最多描述一个行为，建议仅包含：

- 行为与用户可见结果；
- 先失败的测试或复现；
- 最小实现边界与明确不做的事项；
- 本次优先运行的本地测试、OpenAI SDK、独立 Python 脚本或 curl 命令；
- 完成后应更新的实现事实或仍未知的边界。

它不是路线图。新想法、第二个行为或更大范围在获准成为当前焦点前，不写成仓库内实施计划。

## 证据表达规则

- 区分“测试已通过”“SDK/独立客户端已观察到”“目标 Agent 已观察到”“真实 Provider 已观察到”和“尚未验证”；不要把任一项写成整体兼容结论。
- 记录失败时优先保留 request id、脱敏配置、错误分类、是否已收到首个可见输出以及可重跑步骤；不得保存 credential、cookie 或私人内容。
- Provider 限流、429、超时和临时网络失败的行为以[Provider 韧性需求](provider-resilience.md)为设计参照；每次实现都为对应分支先写测试。
- 需要研究的协议或客户端事实，记录来源和适用边界；研究结论不是实现完成的替代品。
