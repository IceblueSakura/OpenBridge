# LiteLLM Responses 与协议转换测试资产调研

## 状态与来源

- 在线复核日期：2026-07-26；当前模块级复核 commit `23de7a15d9d40006ee596e617475ba101d60c5e9`
- 来源：[Responses tests](https://github.com/BerriAI/litellm/tree/23de7a15d9d40006ee596e617475ba101d60c5e9/tests/llm_responses_api_testing)、[base test](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/tests/llm_responses_api_testing/base_responses_api.py)、[issue #20711](https://github.com/BerriAI/litellm/issues/20711)、[issue #25321](https://github.com/BerriAI/litellm/issues/25321)

## 观察事实

- 测试目录覆盖 OpenAI、Azure、Anthropic、Google 等 Provider 的 Responses 请求、stream iterator、hooks 与 tool result。
- 测试深度依赖 LiteLLM 内部类型、Provider adapter、cache 与兼容策略。
- issue #20711 展示 Chat tool-call 首 chunk 带 id、后续 chunk 只带 index 时，缺少 `index -> call_id` 状态会丢 arguments delta。
- issue #25321 展示切换 content block 时丢弃触发 chunk，最终 tool input 变为空的错误类别。
- 部分修复会跳过空 `call_id` 或从 cache 重建缺失调用。

## 覆盖与边界

这些 tests/issues 适合提供多 Provider 字段差异和负面回归样本；内部对象、静默跳过或全局 cache 补全是 LiteLLM 策略，不能被解释为协议标准。Provider adapter tests 也不单独证明双向转换的全部语义。

