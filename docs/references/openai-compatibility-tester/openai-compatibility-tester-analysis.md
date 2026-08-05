# beranekio/openai-compatibility-tester 调研

## 状态与来源

- 在线复核日期：2026-07-26
- 本次在线检查未固定 commit。
- 来源：[openai-compatibility-tester](https://github.com/beranekio/openai-compatibility-tester)

## 观察事实

- 通过官方 OpenAI Go SDK 对任意 HTTP endpoint 执行黑盒兼容测试。
- 默认覆盖 models、Chat、Chat stream、Responses 和 Responses stream。
- 扩展套件增加 tools、errors 等场景。
- SDK 无法解析 payload 或基础校验失败时以非零状态退出，适合 CI smoke。
- 项目提供 canned mock server，但复核时仍较新，test semantics 与稳定性需要固定 commit 后再评估。

## 覆盖与边界

适合 endpoint/SDK-shape smoke 和 Go SDK 消费互证；不检查 source protocol 到 target protocol 的内部转换，也不能替代精确
event 序列、identity、terminal 和错误策略断言。

