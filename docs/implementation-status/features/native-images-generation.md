# Native Images Generations

## 已实现

下游 `POST /v1/images/generations` 作为独立 Images operation 实现，当前只绑定 Bailian/DashScope `qwen-image-3.0` 与 `qwen-image-3.0-pro` 两个 Native target：

- **下游合同（OpenAI-first）**：识别当前 OpenAI Images Create 标准字段；未知字段仍为 `invalid_request_error`，已知但 qwen 不支持的标准字段为字段级 `unsupported_model_capability`。OpenAI optional `null` 视为省略；qwen 支持 `size:"auto"`、固定 PNG 和 `stream:false`。
- **DashScope extension**：兼容 OpenAI SDK `extra_body` 的顶层 `prompt_extend`、`prompt_extend_mode`、`enable_thinking`、
  `negative_prompt`、`seed`、`watermark`；由 typed `DashScopeImagesCapabilities` 约束并投影到 `interfaces.images.dashscope_extensions`。
- **请求转换**：OpenAI prompt/n/size 映射为 DashScope Native；`auto` size 省略，标准下游-only 字段不出网。DashScope 默认显式冻结为
  prompt extension=true/direct、thinking=true、watermark=false，避免依赖上游默认漂移。
- **能力契约**：Provider ceiling `n` 1–6、size 每边 512–2048 且面积 512²–2048²、`response_format:url`、
  `output_format:png` 与 DashScope extension；Public Model 只在全部候选 extension profile 相等时公开扩展。
- **响应验证**：上游 choice URL、`usage.output_image_count`、width、height 在 commit 前完整校验；成功投影为
  `{created, data:[{url}], output_format:"png", size:"宽x高"}`，图片数量/尺寸进入独立 histogram 而非 token usage。
- **执行边界**：单 candidate、无 Bridge、无 fallback、不自动 retry（图像生成请求可能已被计费）；`user` 仅参与
  严格目录校验，不出网。
- **错误矩阵**：400 `invalid_request_error` / `unsupported_model_capability`、404 `model_not_found`、
  413 `request_too_large`、415 `unsupported_media_type`、500 `configuration_error`；非成功上游状态保留 status 但统一脱敏为
  `upstream_error`，当前 transport failure（包括 timeout）统一返回 502 `upstream_error`。
- **观测**：`request_kind="images"`、operation `images_generations`；原始 prompt、上游 body 与 URL 不进入 OTLP。

## 证据

- `tests/images_forwarding_contract.rs`（10 tests）：OpenAI 标准字段分类/null/auto、known-but-unsupported zero-egress、
  DashScope extension profile/default/dependency/wire、choice/usage 双重响应验证与实际 metadata 投影。
- 2026-08-22 在本机 checkout 真实 DashScope 直连验证（loopback gateway，真实 `bailian-primary` credential）：
  - OpenAI-first 请求同时使用 `n:null`、`size:"auto"`、`response_format:null`、`output_format:"png"`、`stream:false` 与
    六个 DashScope extension，`qwen-image-3.0` → 200，返回一张 URL、`output_format:"png"`、实际 `size:"2048x2048"`，耗时约 31s；
  - `quality:"high"`、`stream:true` 在约 10ms 内返回字段级 `unsupported_model_capability`；
    `prompt_extend:false + enable_thinking:true` 返回字段级 `invalid_request_error`，均 zero egress；
  - 扩展 Models 返回完整 `dashscope_extensions` typed defaults/domain，未暴露 Provider/Target/credential。
  - `qwen-image-3.0` T2I `1024x1024` n=1 → 200 `{created, data: [{url}]}`（24h 签名 URL），耗时约 44s；
  - n=2 → 200 两张 URL；
  - 未知字段 → 400；`b64_json` → 400 param=`response_format`；`size=4096x4096` → 400 param=`size`；
    `n=7` → 400 param=`n`；未知模型 → 404。
- 全量基线：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`、
  `git diff --check` 全绿。

## 未证明范围

- 未验证真实 OpenAI `/v1/images/generations` 兼容 SDK、图像内容质量、计费语义或配额边界。
- I2I 编辑、异步任务轮询（`X-DashScope-Async`）、stream 输出与 `b64_json` 未实现。
- transport timeout 尚未独立映射为 504 `upstream_timeout`，Images 也尚未复用共享 execution runner。
- 图像 URL 是 24h 临时签名 URL，OpenBridge 不下载、缓存或延长有效期。
- 未跑负载、长期运行或生产 logging 验证。
