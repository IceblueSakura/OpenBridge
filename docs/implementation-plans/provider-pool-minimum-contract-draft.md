# Provider 池与最小能力契约计划草案

> 状态：本草案的当前范围已执行。Provider 池行为已落地；`current-focus.md` 中的 MiMo 图片工作按用户要求保留，不在本轮替换。

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

## 模型名称规则

1. 定义阶段的 canonical Model ID 保持 `designer/model` 格式，用于标识模型事实的来源和具体模型定义。例如：`z-ai/glm-5.2`。
2. Routing 阶段的模型绑定使用 `provider/model` 格式，用于区分同一模型在不同 Provider 上的执行来源。例如：`openai/gpt-5.6-sol` 与 `chatgpt/gpt-5.6-sol`。
3. 下游标准 Models、扩展 Models、请求字段和响应投影始终使用不带前缀的 `model` 形式。例如：`gpt-5.6-sol`。下游不得看到 `designer/`、`provider/`、Target、Route 或 upstream model 前缀。
4. 同一个裸 `model` 只有在代码目录显式把多个 `provider/model` source 绑定到同一个 Public Model 时才形成 Provider 池；名称相同本身不触发隐式聚合。
5. `provider/model` 是路由层的受信模型身份，不自动等同于 Provider API 请求中实际发送的 `upstream_model` wire 值。两者的映射必须由 Provider registration 固定声明，不能由下游请求或字符串拼接决定。

### 名称边界示例

```text
定义层 canonical Model:  z-ai/glm-5.2
路由层 source:           openrouter/glm-5.2
下游 Public Model:       glm-5.2
实际 upstream_model:     由 OpenRouter Provider registration 固定决定
```

## 当前边界

- 本草案暂以 `gpt-5.6-sol` 为第一个候选，不自动扩大到整个 GPT-5.6 系列。
- 本草案接受因 ChatGPT 订阅上下文更窄而产生的最小公共上下文契约；当前已知目标值应以 ChatGPT profile 的保证上限为下游安全上界。
- 本草案不改变 ChatGPT OAuth 生命周期、Provider adapter、受信 endpoint、credential 文件所有权或下游认证边界。

## 当前范围的执行结果

- `gpt-5.6-sol` 现在由显式 OpenAI source 与 ChatGPT source 组成有序 Provider 池，OpenAI source 保持优先。
- `ModelConfig.id` 和 `UpstreamTargetConfig.canonical_model` 保持 `designer/model`；`UpstreamTargetConfig.provider_model` 和运行时 target routing identity 使用 `provider/model`，并在启动时校验 Provider 前缀与模型 basename。
- 公共 Models、请求和响应继续只使用裸 `gpt-5.6-sol`；实际 `upstream_model` 仍由各 Provider registration 固定提供。
- Provider 池的上下文、能力和参数契约继续由静态可执行候选保守取交集；ChatGPT source 的较窄上下文会收窄公共契约。
- MiMo 图片能力、Native surface 和对应当前焦点没有纳入本次改动范围。

## 待补充和待确认

- Provider pool 在缺少某个 source 的 credential、Target 不可用或启动时被禁用时，Public Model 是否仍必须保持 pool-wide 的最小契约，还是允许按本次启动实际可执行 source 重新计算。
- OpenAI、ChatGPT 的 source 优先级，以及 Chat 与 Responses 两个 operation 是否采用相同优先级。
- Luna、Terra 和其他 GPT-5.6 source 的纳入清单、下游名称和语义等价边界。
- 删除独立 ChatGPT Public Model 名称后的客户端迁移、旧名称是否统一返回 `model_not_found`，以及是否需要在当前实验性阶段直接接受该破坏性变更。
- 最小契约除 context 外需要固定收窄的字段，尤其是 `parallel_tool_calls`、structured outputs、output-token-limit 参数、reasoning output 和 state affinity。

## 本阶段明确不做

- 不扩大到 Luna、Terra 或其他 GPT-5.6 source。
- 不改变 ChatGPT OAuth 生命周期、Provider adapter、endpoint、credential 文件所有权或下游认证边界。
- 不把 `designer/model` 或 `provider/model` 暴露给下游，也不把 `upstream_model` 当作路由身份。
- 不替换或合并当前 MiMo 图片焦点；该内容仍保留在 `current-focus.md`。
