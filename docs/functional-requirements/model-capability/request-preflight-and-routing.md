# 请求预检与禁止能力路由

## 状态

本文是[模型与能力契约域](README.md)的预检模块：定义模型请求的固定预检顺序和禁止的能力路由行为。
其他模块见[模型与能力契约域](README.md)导航。

## 1. 请求预检顺序

模型请求必须遵循固定顺序：

1. 分析请求 operation、Public Model，以及该接口的 input form、encoding/dimensions、streaming/non-streaming delivery、精确 tool choice mode、媒体
   part/source/format/detail、URL 长度、inline 编码/解码字节、task-neutral message shape、闭合 Structured Output request variant、逐值 Responses `include`、reasoning、state 和输出限制等结构事实；
   analyzer 不选择 canonical task、Public Model interface 或 Route。
2. 查询所选 Public Model 的目标接口固定契约。
3. 取得固定接口后才解释 task-specific 音频 shape，并对所有已建模请求能力执行一次 fail-closed 预检；VoiceClone conditioning 保持独立，
   specialist audio 的额外、空或角色错误 message 不得进入 RoutePlan。
4. generation 请求若需要正向 reasoning level 归一化，只在该固定接口上解析并改写一次 canonical body；字段缺失、Responses
   `reasoning: {}` 和已精确支持的值保持原字节，`none` 不参与正向归一化。
5. 不支持或未知时立即返回错误，不创建 RoutePlan，不调用 Provider adapter 或 transport。
6. 预检通过后，严格按 Public Model 的配置顺序构造完整 RoutePlan，全部 fallback candidate 使用同一归一化结果。

代码目录从多个 Provider source 生成配置顺序时，必须使用 Public Model 显式声明的 `NativeFirst` 或 `SourceFirst` 类型化策略；
生成后这一 Vec 即为固定配置顺序，运行时不得再按 Provider 或模式重排。无论采用哪种策略，全部静态候选都参与固定能力交集；某条
streaming-only Route 禁用转换时，不得为了满足非流式请求而跳过它。

## 2. 禁止的能力路由

以下行为一律禁止：

- 根据请求能力选择另一个 Public Model；
- 因某条 Route 能力较弱而跳过它；
- 因后续 Route 能力较强而提升公共契约；
- 因某个 function tool choice 或 structured-output mode 只被后续 Route 支持而跳过前序 Route；
- 根据能力、模型字符串、价格、健康或 benchmark 重排 Route；
- 把一条 Route 的 tool、image、reasoning 或 token 优势与另一条 Route 的能力做字段并集。

Route 候选资格只取决于协议匹配和静态启停；Target/API 绑定、顺序及 `Native`/`Bridged` 模式均来自固定配置。Public Model 的 reasoning
输入归一化发生在 RoutePlan 构造前；Provider reasoning wire 映射只能在选定候选的 egress 请求准备阶段改写 wire 副本，不得写入
RoutePlan，也不能改变候选资格或顺序。若完整
`BridgePlan` 无法表示已通过 公共预检的请求，整个请求必须失败，不能跳过该 Bridge 去选择其他 Route。 运行期
cooldown、429/5xx、timeout、credential rotation 和首输出前 fallback 属于可用性执行，不是能力路由；
只有请求实际携带 `previous_response_id` 时才禁止跨 Target fallback；候选具备 continuation 能力本身不能改变无状态请求的 fallback，
state ownership 也不能选择能力更强的候选。

## 关联文档

- [模型与能力契约域导航](README.md)
- [模型事实与固定接口契约](model-facts-and-interface-contract.md)
- [路由与 Provider 韧性](../routing-resilience/provider-resilience.md)
- [网关 API 域：请求与安全边界](../gateway-api/request-and-security-boundary.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
