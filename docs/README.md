# OpenBridge 文档索引

本文只负责文档分类、阅读入口和维护规则。当前 checkout、源码、确定性测试和明确记录的实际验证是实现事实的依据；需求、计划或外部
参考不能自动证明代码已经支持某项能力。

## 1. 按目标选择入口

| 目标 | 从这里开始 | 继续阅读 |
|---|---|---|
| 安装、配置和调用 | [根 README](../README.md) | 配置模板、OpenAPI、常见问题 |
| 判断产品应保持什么行为 | [功能需求](functional-requirements/README.md) | 对应功能域的合同、失败语义与非目标 |
| 判断当前代码已实现什么 | [实施现状](implementation-status/README.md) | 当前实现、架构、映射、边界与 evidence |
| 查看当前保留的实施优先级 | [实施计划入口](implementation-plans/README.md) | [当前开发焦点](implementation-plans/current-focus.md)及对应需求、测试和状态页 |
| 核验外部协议或 Provider 事实 | [参考资料](references/README.md) | 固定 source snapshot 与重新核验边界 |
| 查看当前实现与源码 owner | [当前实现](implementation-status/current-state.md) | [当前代码架构](implementation-status/current-architecture.md) |
| 查看 Model 与 Provider 的当前映射 | [Model 与 Provider 映射](implementation-status/model-provider-mapping.md) | Provider Target 与 Public Model 注册 |
| 查看未实现与未验证范围 | [当前状态边界](implementation-status/current-boundaries.md) | 带日期的外部 evidence |
| 设计或运行项目语义测试 | [Semantic testing](../testdata/semantic-testing.md) | [评测方法证据](references/semantic-testing-methods.md)与 corpus/testkit |

## 2. 文档类别与唯一职责

| 类别 | 只回答什么 | 不应包含什么 |
|---|---|---|
| `functional-requirements/` | 产品行为、客户端结果、安全边界、非目标和验收约束 | 当前测试结果、实现日志、候选设计 |
| `implementation-status/` | 当前 checkout 已实现的事实、源码 owner、已执行证据和未证明边界 | 未获准路线图、外部协议全文 |
| `implementation-plans/` | 当前保留的开发焦点与优先级 | 第二份路线图、完成历史、状态快照 |
| `references/` | 外部协议、SDK、Provider、客户端和参考项目事实 | OpenBridge 当前实现、产品承诺或实施步骤 |

本地实现理由优先写在模块/API 文档中；只有形成跨模块产品合同或实施事实时，才进入上述文档。

文档不维护单模型 context、模态、tokenizer、reasoning、参数或价格副本。对于可直接从 official website 或 OpenRouter 获取的信息，优先记录来源 URL、来源身份、最后复核日期和重新核验条件，不复制完整 payload 或能力表。当前映射只记录 Model、Provider Target 与 Public Model 关系；模型能力由代码和运行中的扩展 Models API 自描述，外部动态事实由官方文档描述。

只有已执行测试与所引用的 official/OpenRouter 声明不一致时，才新增带日期 evidence；记录必须只描述来源声明与实际观察的差异，并保留 endpoint、model ID、payload、账户/地域/网络边界和“不证明什么”。目录之间的字段差异、缺失字段或未经请求验证的推论不能写成已验证差异。

## 3. 运行时契约资产

[openapi.yaml](openapi.yaml)和 [swagger-ui.html](swagger-ui.html)会由服务编译并分别通过 `/openapi.yaml` 与 `/swagger-ui/` 交付。
它们不是生成后的附属文件：接口行为变化时必须与源码、serialization、错误、示例和测试原子更新。

OpenAPI 描述当前 system 与 OpenAI-compatible HTTP surface，不包含 MCP dual-era transport；MCP 由
[网关 API 合同](functional-requirements/gateway-api.md)及对应 transport tests 拥有。OpenAPI 也不表示每个 Public Model 支持所有
可选字段；具体模型能力以运行中的 `/openbridge/v1/models` 固定接口契约为准。

## 4. 功能专题入口

| 问题域 | 需求 | 当前实现或证据 |
|---|---|---|
| 产品范围与部署边界 | [产品范围](functional-requirements/product-scope.md) | [当前实现](implementation-status/current-state.md) |
| 网关 endpoint、认证、JSON/SSE、MCP | [网关 API](functional-requirements/gateway-api.md) | [当前实现](implementation-status/current-state.md) |
| Public Model、Models API、能力预检 | [模型能力](functional-requirements/model-capability.md) | [当前实现](implementation-status/current-state.md) |
| Bootstrap、用户、API key 与 OAuth | [配置与凭证](functional-requirements/configuration-credentials.md) | [当前实现](implementation-status/current-state.md) |
| Route ordering、retry/fallback、cooldown | [路由与韧性](functional-requirements/routing-resilience.md) | [当前实现](implementation-status/current-state.md) |
| Embeddings、图片、文件与音频 | [扩展能力](functional-requirements/extended-capabilities.md) | [当前状态边界](implementation-status/current-boundaries.md) |
| 本地内容日志与 OpenTelemetry | [观测需求](functional-requirements/observability.md) | [当前实现](implementation-status/current-state.md) |
| Provider 当前接入 | 对应产品/能力需求 | [当前实现](implementation-status/current-state.md#7-provider-注册) |
| 外部 OpenAI/Provider/项目事实 | 不构成需求 | [参考资料](references/README.md) |

## 5. 变更工作流

行为变更开始前：

1. 从当前源码、工作树、功能需求和实施状态建立基线；
2. 明确用户可观察结果、失败语义、安全/资源边界和不做项；
3. 在[当前开发焦点](implementation-plans/current-focus.md)中记录获准范围；
4. 先建立失败测试、fixture 或最小客户端复现，再做最小实现；
5. 先运行 focused validation，再运行与改动相称的仓库基线；
6. 把确认事实与实际命令写入实施状态或 evidence，更新所有受影响的单一事实 owner。

未发布原型可以在获准焦点内直接修正 API、Bootstrap、fixture 或内部模块，但不得因此读取、重写或提交私有配置，也不得保留无意义的
legacy alias、双实现、猜测式迁移或兼容垫片。

## 6. 证据表达

必须分别标明以下层次：

1. 静态源码或 schema 检查；
2. 确定性 Rust test / fixture；
3. Python corpus/testkit 或独立 loopback；
4. 外部 SDK 或独立 curl/Python 客户端；
5. 目标 Agent runtime；
6. 真实 Provider；
7. 负载、长时间运行或生产环境。

低层证据不能替代高层验收，真实 Provider 一次成功也不能替代可重复回归。每份 evidence 应记录时间及时区、checkout、工作树状态、
工具版本、脱敏配置形状、实际范围、结果和“不证明什么”，不得保存 credential、Cookie、私人正文或 Provider request ID。

新 endpoint 可以先使用仅存在于 test/fixture 的 synthetic Provider、loopback upstream 或 resource/session simulator 建立 fake contract；
它必须经过真实下游 router、认证、limit、analysis、planning、transport、renderer、错误和终态观测路径。fake 成功只证明相应 wire 或
state-machine，不得进入 production `/v1/models`，也不证明真实 Provider、模型质量、费用、保留策略或负载能力。

## 7. 参考资料元数据

每份 reference 应能独立说明：

- source 与 source snapshot；
- last reverified；
- 阅读范围与证据边界；
- 重新核验触发条件。

动态 endpoint、SDK、模型、价格、beta 和 deprecation 事实必须按快照理解；真正实施前重新核验。综合文档必须链接其项目级前置证据。

## 8. 文档验证

纯文档变更至少检查：

- Markdown 相对文件链接与本地锚点；
- 每个文档和非 Markdown 快照是否有可达 owner；
- requirements 是否混入实施事实，status 是否混入候选计划，references 是否混入本地当前状态；
- 模型数量、测试数量、Provider 清单和“最近证据”是否只有一个 owner；
- `git diff --check`。

只有文档调整涉及运行时资产、serialization、OpenAPI 交付路径或产品行为时，才追加对应 Rust focused tests 和完整基线。
