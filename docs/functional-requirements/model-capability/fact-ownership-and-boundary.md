# 事实所有权与公开边界

## 状态

本文是[模型与能力契约域](README.md)的事实所有权模块：定义 Canonical Model、Provider/Upstream API、Route、
Public Model 与 RoutePlan/attempt 各层拥有的事实和公开边界。其他模块见[模型与能力契约域](README.md)导航。

## 1. 事实所有权

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

## 关联文档

- [模型与能力契约域导航](README.md)
- [Public Model 身份、生命周期与可见性](identity-and-lifecycle.md)
- [模型事实与固定接口契约](model-facts-and-interface-contract.md)
- [实施现状](../../implementation-status/README.md)
