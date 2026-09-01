# 实体、事实所有权与身份生命周期
本文只记录目标行为、失败语义与安全边界，不记录实现完成度或测试结果。

### 域边界

本域是 Public Model 身份、模型信息、固定接口能力、请求预检和 Models API 的唯一需求入口。Route 执行、
retry、fallback 与 cooldown 见[路由与 Provider 韧性](../routing-resilience.md)；实现与验证事实见
[实施现状](../../implementation-status/README.md)。

### 域目标（用户结果）

客户端只需选择一个稳定 Public Model 和 Chat Completions、Responses 或 Embeddings 接口，即可在发起模型请求前读取同一份
静态能力契约。若所选模型不支持请求能力，OpenBridge 必须在任何上游调用前返回稳定错误；不得自动改选模型或寻找能力更强的 Route。
只有[普通参数上游兼容规则](../gateway-api/parameter-compat.md)中的闭合字段可以在选中 Upstream API 的
egress 边界静默删除，其他请求字段不得被隐式降级。

模型信息用于能力展示和正确拒绝，不承担模型推荐、质量排序、成本优化或运行时调度。

### 1. 事实所有权

| 层次                    | 拥有的事实                                                                          | 是否向下游公开                                     |
|-------------------------|-------------------------------------------------------------------------------------|----------------------------------------------------|
| Canonical Model         | 与 endpoint/credential 无关的公共 identity envelope，以及必填 task union 所拥有的上下文、模态、参数和 reasoning 事实；已核实的 ChatGPT subscription profile 与一般 API 事实不同时，可使用独立 canonical profile identity | 模型事实经 Public Model 聚合；参数只经接口契约公开 |
| Provider / Upstream API | Provider operation 能力上界；音频与 Responses state ceiling 分别和单个 Target executable profile 静态分型；另拥有 served limits、协议、upstream model、state ownership 和 wire 映射 | 否                                                 |
| Route                   | 下游协议、Target、Upstream API、`Native`/`Bridged` 模式及配置顺序                   | 否                                                 |
| Public Model            | 稳定身份、生命周期、模型事实和每协议唯一固定能力契约                                | 是                                                 |
| RoutePlan / attempt     | 已接受请求的执行顺序、retry、fallback、credential 与 cooldown 状态                  | 否                                                 |

公共模型对象不得包含 Provider、Target、Route、upstream/canonical model id、endpoint、credential、header 或 wire
mapping，也不得包含健康、延迟、配额、价格、成本、指标、排行或 benchmark。运行指标通过独立的 startup-owned OTLP metrics
signal 导出，不属于 `PublicModelInfo`；上游 `/models` 与 probe 结果不能自动注册或扩大 Public Model。

Canonical profile identity 只用于区分不同的已核实模型事实，不代表 endpoint、credential 或请求方可选择的 Provider；其具体可调用性
仍必须由显式 Target、Upstream API、Route 和 Public Model 注册形成。

每个 canonical Model 必须选择且只选择一个闭合 task variant：`Generation`、`Embedding`、`SpeechRecognition`、
`SpeechSynthesis`、`VoiceDesign` 或 `VoiceClone`。公共 identity envelope 不复制 task payload；context、modalities、ordinary
parameters 和 canonical reasoning 只能由所选 variant 拥有或派生。不得重新引入平铺 task 字段、多个 bool、空 payload 或第二套可独立
修改的 task 状态。

### 1. 身份、生命周期与可见性

- `id` 是客户端请求和资源路径使用的稳定单段标识，格式为
  `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}`；包含 `/` 的上游模型名不得直接成为 Public Model id。
- `created` 是 Public Model 契约首次创建的稳定 Unix 秒，不使用进程启动时间。
- `name`、可选 `description` 和 `lifecycle` 是面向客户端的静态元数据。
- `active` 与 `deprecated` 模型仍可列出和调用；`retired` 模型对 list、retrieve 和模型请求统一表现为不可用。
- 没有任何静态可执行 Chat/Responses/Embeddings 接口的 Public Model 不进入可见目录。
- 标准列表、扩展列表、两个 retrieve 接口和请求预检必须读取同一个不可变 registry snapshot。
