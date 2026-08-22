# Native Images Generations

## 已实现

下游 `POST /v1/images/generations` 作为独立 Images operation 实现，当前只绑定 Bailian/DashScope `qwen-image-3.0` 与 `qwen-image-3.0-pro` 两个 Native target：

- **下游契约**（OpenAI 兼容）：严格字段目录 `model`、`prompt`、`n`、`size`、`response_format`、`user`；未知顶层字段在 egress 前以 `invalid_request_error` 拒绝。
- **请求转换**：OpenAI `{prompt, n, size, response_format}` 在 Provider adapter 内转换为 DashScope 原生
  `{input: {messages: [{role: "user", content: [{text}]}]}, parameters: {n, size}}`；`size` 分隔符由 `x` 改写为 `*`，
  `user` 与 `response_format` 不出网关。
- **能力契约**：Provider ceiling `n` 1–6、size 每边 512–2048 且面积 512²–2048²、`response_format` 仅 `url`；
  Public Model `interfaces.images` 投影为同源保守交集，preflight 解析 `n`/`size`/`response_format` 后固定。
- **响应验证**：上游 DashScope envelope 在 commit 前完整校验——按 body `code` 字段识别业务错误、逐 choice 提取
  非空 `image` URL、确认数量等于已解析 `n`、投影为 `{created, data: [{url}]}`；bounded JSON budget 内。
- **执行边界**：单 candidate、无 Bridge、无 fallback、不自动 retry（图像生成请求可能已被计费）；`user` 仅参与
  严格目录校验，不出网。
- **错误矩阵**：400 `invalid_request_error` / `unsupported_model_capability`、404 `model_not_found`、
  413 `request_too_large`、415 `unsupported_media_type`、5xx `upstream_error`/`upstream_timeout`/`configuration_error`。
- **观测**：`request_kind="images"`、operation `images_generations`；原始 prompt、上游 body 与 URL 不进入 OTLP。

## 证据

- `tests/images_forwarding_contract.rs`（5 tests）：严格目录拒绝、b64_json 拒绝、size/n 超域 zero-egress、
  OpenAI→DashScope wire 转换、数量不匹配 fail-closed、端到端 loopback。
- 2026-08-22 在本机 checkout 真实 DashScope 直连验证（loopback gateway，真实 `bailian-primary` credential）：
  - `qwen-image-3.0` T2I `1024x1024` n=1 → 200 `{created, data: [{url}]}`（24h 签名 URL），耗时约 44s；
  - n=2 → 200 两张 URL；
  - 未知字段 → 400；`b64_json` → 400 param=`response_format`；`size=4096x4096` → 400 param=`size`；
    `n=7` → 400 param=`n`；未知模型 → 404。
- 全量基线：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`、
  `git diff --check` 全绿。

## 未证明范围

- 未验证真实 OpenAI `/v1/images/generations` 兼容 SDK、图像内容质量、计费语义或配额边界。
- I2I 编辑、异步任务轮询（`X-DashScope-Async`）、stream 输出与 `b64_json` 未实现。
- 图像 URL 是 24h 临时签名 URL，OpenBridge 不下载、缓存或延长有效期。
- 未跑负载、长期运行或生产 logging 验证。
