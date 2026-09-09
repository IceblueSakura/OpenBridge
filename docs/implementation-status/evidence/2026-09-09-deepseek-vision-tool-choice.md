# DeepSeek Vision tool choice 差异复测

## 来源与执行边界

- 时间：2026-09-09 10:13:14–10:13:59，Asia/Shanghai（UTC+08:00）。
- Checkout：`22e3ebbab61cf28a11d8b9392f9674029c3861c1`。执行时仅当前焦点文档有修改，Provider 注册尚未修改；probe 从该源码重新 release 构建。
- 工具：cargo 1.97.1、rustc 1.97.1、Python 3.14.7。
- 来源：[DeepSeek 官方 Responses guide](https://api-docs.deepseek.com/guides/responses_api/)，本次重新读取的声明为 `tool_choice Supported. none / auto / required / a specific tool`。
- 目标：官方直连 `deepseek-v4-flash-vision-exp`；Chat `/chat/completions`、Responses `/responses`。沿用本机已配置账号与网络，不读取或记录密钥、账号标识、IP；未跨账号或地域复测。

## 请求与观察

经用户确认，使用 `tools/probe/matrix.py` 编排 `tool-auto/tool-none/tool-required/tool-named` × Chat/Responses × JSON/SSE，共 16 次独立请求，无 retry/fallback。

请求使用现有固定 synthetic prompt、两个 function tools 与固定 `value` schema；省略 reasoning，不发图片、不执行工具、不续轮。每次请求的 output limit 为 4096 tokens，Target timeout 为 120 秒，子进程上限 150 秒，间隔 2 秒。报告不保存生成正文、工具参数、call ID 或 Provider request ID。

| 模式 | Chat JSON/SSE | Responses JSON/SSE |
|---|---|---|
| auto | 均 200，固定 oracle supported | 均 200，固定 oracle supported |
| none | 均 200，固定 oracle supported | 均 200，固定 oracle supported |
| required | 均 400 | 均 400 |
| named | 均 400 | 均 400 |

auto 的响应包含符合固定名称和参数的工具调用；none 没有工具调用。成功请求的报告 usage 合计 input 2356、output 544、total 2900 tokens（其中 reasoning 284，已包含在 output 中）；400 响应未报告 usage，不能据此推算完整账单。原始脱敏报告位于 gitignored `testdata/runtime/deepseek-vision-tool-choice-review/`，不入库。

## 结论与不证明什么

本次确认该 Target 在固定请求边界下与官方通用声明存在差异，因此只将其 Chat/Responses tool choice 收窄到 auto/none；不改 Provider ceiling、canonical Model 或其他 Target。注册处引用本记录，确定性 Router 回归验证公开模式、required/named 的 zero-egress 拒绝、auto/none 的 wire 保留及其他 Target 不被全局收窄。

旧矩阵中的 strict/parallel case 同时发送 required，不能把其 400 独立归因到 strict/parallel；本次不复测、不调整它们。结果不证明所有 reasoning 组合、其他账号/区域/模型、完整 SDK 工具续轮、模型质量、长期稳定性或费用。后续放宽需重新核对来源并执行独立复测，不改写本次历史观察。
