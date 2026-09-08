# OpenBridge 文档

按阅读任务选择入口。产品契约说明“应当怎样工作”；源码与测试说明当前实现；带日期的执行证据说明“在什么范围内实际验证过”。三者有分歧时先核对，不自动相互覆盖。

## 从这里开始

| 你的目标 | 阅读入口 |
|---|---|
| 安装、配置和调用 | [使用手册](../README.md)、[Provider 探测指南](guides/provider-probing.md) |
| 理解产品行为与失败语义 | [功能需求](functional-requirements/README.md) |
| 理解模块职责与请求数据流 | [当前架构](architecture.md) |
| 修改代码并选择验证方式 | [开发指南](development.md) |
| 判断当前实现与验收差距 | [实施现状](implementation-status/README.md) |
| 定位模型注册和 Provider 接入边界 | [映射](implementation-status/model-provider-mapping.md)、[Provider 状态](implementation-status/providers/README.md) |
| 核对一次外部验证 | [验证证据](implementation-status/evidence/README.md) |
| 核对外部协议、SDK 或参考项目 | [参考资料](references/README.md) |
| 查看当前获准工作范围的记录 | [当前工作范围](implementation-plans/README.md)与[当前开发焦点](implementation-plans/current-focus.md) |
| 设计或运行语义测试 | [Semantic testing](../testdata/semantic-testing.md) |

## 文档职责

| 位置 | 维护的事实 | 不应混入 |
|---|---|---|
| 根 `README.md`、`guides/` | 配置、调用、操作与排障说明 | 完整架构、完成日志 |
| `functional-requirements/` | 当前有效的行为、安全与资源约束、非目标、验收要求 | 当前模型接线、测试通过记录 |
| `architecture.md` | 稳定职责、依赖方向、关键数据流和跨模块选择理由 | 内部算法、版本常量、库存表 |
| `development.md` | 变更流程、测试职责、验证命令与交付条件 | 当前任务进度、产品契约正文 |
| `implementation-status/` | 当前实现范围、差距、注册关系、Provider 特有边界和证据指针 | 产品规则副本、完成流水账 |
| `implementation-status/evidence/` | 有独立价值的实际验收与差异观察及其固定边界 | 当前能力保证、未经执行的推论 |
| `implementation-plans/current-focus.md` | 用户已经批准的短周期行为范围及验证边界 | 自动授权、候选路线图、完成历史 |
| `references/` | 外部来源的固定协议、SDK、Provider 与参考项目事实 | OpenBridge 实现状态或实施授权 |
| 根 `AGENTS.md` | Agent 授权、安全、工作纪律和按需阅读入口 | 另一套产品说明或模块地图 |

实现细节与局部理由由源码 `//!`/`///` 注释和测试维护。这里呈现收敛后的当前设计，不维护推测性路线图或 ADR 决策演进历史。

## 内容与可读性

- 一个详细事实有一个明确维护位置；其他页面可用短摘要和链接，不复制长规则、精确清单或状态表。
- 标题按问题域命名，开头简述范围。表格用于导航和对照，不强制每页套用空章节。
- 区分“必须/不得”的产品约束、“已实现”的代码事实与“已验证”的执行结果。
- 产品永久非目标归需求；范围内缺口与未验证项归状态。状态不是未来实施计划。
- 保持既有验收 ID 和技术标识；索引不维护文档数量、模型数量、测试数量或“最新”快照。
- 代码注册关系可以维护为实现映射；单模型 capability metadata、价格和外部全量目录不复制到文档。能力事实由代码、扩展 Models API 或外部官方来源拥有。扩展 Models API 不公开私有执行拓扑，不能替代维护者的注册源码入口。
- 大合同域按独立职责拆叶子；只在确有导航价值时增加目录 README，不按文件长度机械拆分。

## 证据与来源

实际执行的接入验收与实测差异，只有具有独立、持续参考价值时才进入 [evidence](implementation-status/evidence/README.md)；不要求每次成功探测生成文档。差异记录必须保留准确来源声明与观察差异。目录字段分歧或未经请求验证的推论不能写成已验证行为差异。

证据按当时日期、checkout、工具版本及账号/区域/网络/payload 范围解释，不承诺当前可达或长期兼容。不保存凭据、Cookie、账号标识、私人正文、Provider request ID 或完整敏感请求响应。后续实现改变时更新状态解释，不把历史记录改写成当前结论。

外部资料必须记录来源 URL/身份、source snapshot、last reverified、阅读范围、证据边界和 recheck trigger，具体规则见 [references](references/README.md)。本地整理或链接检查不刷新外部复核日期。采用动态协议、SDK 或 Provider 事实前重新核验。

静态检查、确定性 Rust、Python/loopback、外部 SDK、Agent、真实 Provider、负载/长期运行分别报告，低层不替代高层。完整验证流程见[开发指南](development.md)。

## 运行时契约资产

[openapi.yaml](openapi.yaml) 与 [swagger-ui.html](swagger-ui.html) 由服务编译交付；不是可任意移动的普通说明文件。接口变化时与实现、serialization、示例、fixture 和测试同步更新。

OpenAPI 描述 system 与 OpenAI-compatible HTTP surface，不包含 MCP dual-era transport；MCP 由[网关 API 合同](functional-requirements/gateway-api.md)与对应测试维护。OpenAPI 不表示所有 Public Model 支持每个可选字段，具体模型接口由运行中的 `/openbridge/v1/models` 描述。

## 修改文档时

先判断改的是产品承诺、当前事实、外部证据还是操作说明，再修改对应维护位置。移动文件时检查仓库内引用、相对链接与锚点，保留必要入口和独立证据，不创建旧路径兼容副本。

完成时检查内容与示例一致、链接和资产归属可达、旧路径已清理，并执行 `git diff --check`。纯文档维护不制造行为焦点；涉及产品或运行时资产变化时按[开发指南](development.md)追加验证。
