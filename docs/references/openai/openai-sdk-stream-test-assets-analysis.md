# OpenAI SDK streaming consumer 资料调研

## 状态与来源

- 在线复核日期：2026-07-26
- 本次只阅读 SDK 默认分支资料，未固定 release/commit。
-
来源：[openai-node streaming helpers](https://github.com/openai/openai-node/blob/main/helpers.md)、[openai-python Chat streaming implementation](https://github.com/openai/openai-python/blob/main/src/openai/lib/streaming/chat/_completions.py)

## 观察事实

- SDK helper 展示 client 如何解析与聚合 Chat streaming chunks。
- Chat accumulator 需要维护 content、tool-call index/id 和 arguments delta 的累计状态。
- SDK 测试的主要目标是客户端对象与事件 API，而不是代理或跨协议转换器。

## 覆盖与边界

SDK 可证明特定版本客户端能否消费一个 wire response，并帮助识别 accumulator 所需字段。它不提供跨协议 golden oracle，也不证明
Provider、gateway 或其他 SDK 的完整兼容性。使用时必须固定 SDK 版本。

