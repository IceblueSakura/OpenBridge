# OpenAI gpt-oss compatibility-test 调研

## 状态与来源

- 在线复核日期：2026-07-26
- 本次在线检查未固定 commit；运行或复制 case 前必须重新 pin 版本并核对许可证。
- 来源：[compatibility-test](https://github.com/openai/gpt-oss/tree/main/compatibility-test)、
  [`cases.jsonl`](https://github.com/openai/gpt-oss/blob/main/compatibility-test/cases.jsonl)、
  [`runCase.ts`](https://github.com/openai/gpt-oss/blob/main/compatibility-test/runCase.ts)、
  [官方说明](https://developers.openai.com/cookbook/articles/gpt-oss/verifying-implementations#quick-verification-of-tool-calling-and-api-shapes)

## 观察事实

- 测试使用 TypeScript Agents SDK 及其底层 OpenAI client。
- Provider 配置可选择 Chat 或 Responses，并可启用 streaming。
- case 由 prompt、工具列表、预期工具和可选参数组成，结果依赖模型是否按 prompt 选择工具。
- 主要检查 API shape、工具选择与参数；完整运行会重复 case 观察一致性。
- README 明确说明 Chat API events 当前未被测试。
- 官方说明把它定位为 basic function calling/API shape smoke，而非完整 OpenAI API 合规认证。

## 覆盖与边界

适合：真实模型或兼容 Provider 的 SDK/API-shape smoke，以及基本 function calling 回归。

不覆盖：确定性跨协议转换、任意 SSE bytes 分片、复杂并行 call 交错、完整 terminal/cancel 或 fault injection。分别运行 Chat 与
Responses 也不证明二者语义转换正确。

对应协议 owner 见 [Chat Function tools](chat-completions-function-tools.md)与
[Responses Function tools](responses-function-tools.md)。
