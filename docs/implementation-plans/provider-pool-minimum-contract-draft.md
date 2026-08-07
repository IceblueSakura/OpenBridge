# Provider 池与最小能力契约计划草案

> 状态：需求记录草案，尚未进入当前开发焦点，也不表示已授权实施。本文件等待其他需求补充后，再整理为一个可实施的可观察行为并并入 `current-focus.md`。

## 目标

将兼容的 GPT-5.6 Provider source 组织为一个有序的 Provider 池，并以一个稳定的下游 Public Model 身份提供服务。当前已明确的候选是把 OpenAI API 与 ChatGPT 订阅侧的 GPT-5.6 Sol 统一暴露为 `gpt-5.6-sol`；Luna、Terra 及其他 GPT-5.6 模型是否纳入本范围待后续补充需求确认。

ChatGPT 订阅 profile 与一般 API profile 的已核实事实继续分别保存；共享下游名称不等于合并 canonical Model identity，也不向客户端暴露 Provider、Target、Route、upstream model 或 credential 信息。

## 需求

1. 一个下游 Public Model 可以显式绑定多个受信 Provider source，并按固定 source 顺序形成候选池。下游请求只能选择 Public Model，不能选择 Provider、Route、endpoint、credential 或候选顺序。
2. 同一 Public Model 的每个下游 operation 使用所有静态可执行 source 的一个固定公共契约。更强 Provider 的能力不得扩大该公共契约，也不得在请求期按能力跳过较弱 source。
3. 上下文、输入和输出 token 上限按所有纳入契约的 source 取安全最小值；布尔能力、参数、模态、reasoning level 和其他集合只公开所有 source 的交集；无法确认的事实保持未知并 fail closed。
4. Provider source 的优先级和 Native-first/Bridge-second 展开顺序在启动编译时固定。retry、credential rotation 和 fallback 只能沿该顺序执行，并且只允许发生在首个下游业务输出之前。
5. 每条 source 继续使用自己的 canonical profile、Target、Upstream API、upstream model、协议转换和 credential pool；下游响应中的模型身份统一投影为 Public Model 名称。
6. 共享名称只用于语义和接口契约足够兼容的 Provider。ChatGPT 的 SSE、Responses-only Native surface、参数限制、reasoning 和状态能力差异必须纳入最小公共契约，不能依靠下游自行猜测 Provider 差异。

## 当前边界

- 本草案暂以 `gpt-5.6-sol` 为第一个候选，不自动扩大到整个 GPT-5.6 系列。
- 本草案接受因 ChatGPT 订阅上下文更窄而产生的最小公共上下文契约；当前已知目标值应以 ChatGPT profile 的保证上限为下游安全上界。
- 本草案不改变 ChatGPT OAuth 生命周期、Provider adapter、受信 endpoint、credential 文件所有权或下游认证边界。

## 待补充和待确认

- Provider pool 在缺少某个 source 的 credential、Target 不可用或启动时被禁用时，Public Model 是否仍必须保持 pool-wide 的最小契约，还是允许按本次启动实际可执行 source 重新计算。
- OpenAI、ChatGPT 的 source 优先级，以及 Chat 与 Responses 两个 operation 是否采用相同优先级。
- Luna、Terra 和其他 GPT-5.6 source 的纳入清单、下游名称和语义等价边界。
- 删除独立 ChatGPT Public Model 名称后的客户端迁移、旧名称是否统一返回 `model_not_found`，以及是否需要在当前实验性阶段直接接受该破坏性变更。
- 最小契约除 context 外需要固定收窄的字段，尤其是 `parallel_tool_calls`、structured outputs、output-token-limit 参数、reasoning output 和 state affinity。

## 本阶段明确不做

- 不修改 Rust 注册、模型目录、Route、Provider adapter、配置、OpenAPI 或测试。
- 不运行 cargo、Python、真实 Provider、外部 SDK 或浏览器验证。
- 不把本草案视为当前 MiMo 图片焦点的替代，也不据此开始实现；完成其他需求补充后，再按单一可观察行为建立失败测试和验证边界。
