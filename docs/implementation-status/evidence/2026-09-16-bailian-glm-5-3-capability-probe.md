# Bailian GLM-5.3 接入前能力探测

## 来源与执行边界

- 时间：2026-09-16，Asia/Shanghai（本次会话内执行并即时记录，记录创建于 17:51）。
- Checkout：`e6c29bc` 之上的工作区修订（含 `deepseek-v4.1-flash` 的 Bailian 上游 ID 更新）；执行时 `bailian/glm-5-3` 尚未注册，探测经 `openbridge-probe --model glm-5.3` candidate 方式借道 Bailian provider 路径。
- 工具:`openbridge-probe`（cargo 1.97.1、rustc 1.97.1）。沿用本机已配置账号与网络，不读取或记录密钥、账号标识、IP。
- 目标：Model Studio `glm-5.3`（`/models` 可见性 + Chat `/chat/completions` 与 Responses `/responses` 固定 case）。端点同时列出 `ZHIPU/GLM-5.3`；本次按官方小写 ID 使用 `glm-5.3`。

## 请求与观察

经用户确认，使用 `openbridge-probe` 逐次调用编排：`models --model glm-5.3` 可见性检查，以及 Chat/Responses 各自的 text、streaming text、tool-auto/none/required/named/strict/parallel-false/parallel-true、json-object、json-schema、json-schema-strict case，共 25 个请求（1 models + 12 chat + 12 responses），无 retry/fallback。每次请求使用工具固定 synthetic prompt、两个 function tools 与固定 schema，output limit 4096 tokens；不执行工具、不发图片、不做 continuation，不保留生成正文、工具参数、call ID 或 Provider request ID。

`--model glm-5.3` 在 `/models` 的 252 个 ID 中可见（`requested_model_listed: true`）。

| case | Chat | Responses |
|---|---|---|
| text（非流式）/ streaming text | 200，oracle supported | 200，oracle supported |
| tool auto | 200，1 次调用，工具与参数精确匹配 | 200，1 次调用，精确匹配 |
| tool none | 200，0 调用 | 200，0 调用 |
| tool required | 200，1 次调用 | **400 拒绝** |
| tool named | 200，0 调用（未执行） | 200，0 调用（未执行） |
| tool strict | 200，0 调用（未执行） | 200，1 次调用，arguments 不符合 strict schema |
| parallel false / true | 200，1 / 2 次调用 | **均 400 拒绝** |
| json-object | 200，合法 JSON object | 200，响应非 JSON object（未执行） |
| json-schema / json-schema-strict | 200，合法 JSON object 但不符合固定 schema | 200，响应非 JSON object（未执行） |

接受请求的报告 usage 合计 input 2,963、output 2,254、total 5,217 tokens（其中 reasoning 1,900，已包含在 output 中）；三个 400 未报告 usage。原始脱敏报告位于本机临时目录（`/tmp/glm53/`，不入库）。

## 结论与不证明什么

按本次固定请求边界，GLM-5.3 以 **Chat-only** 接入 `bailian/glm-5-3`，Chat 面收窄为 `choice_modes=[none, auto, required]`、`parallel_calls=true`、`strict_schema=false`、structured=`JsonObject`。Responses 端点存在（text/streaming 与 auto/none 工具可用），但 required 与 parallel 字段被 400 拒绝、named/strict 与 structured output 不执行，工具语义不完整，本次不接入。

结果不证明其他账号/区域/模型、完整工具续轮、模型质量、多模态、reasoning 参数实际生效、负载、长期稳定性或费用。后续放宽需重新核对来源并执行独立复测，不改写本次历史观察。
