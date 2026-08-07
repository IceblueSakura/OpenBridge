# 功能：`mimo-v2.5` Chat/Responses Native 图片输入

## 状态

**已完成（当前 checkout）。** `mimo-v2.5` 的 Chat Completions 与 Responses 固定接口可以接收受限的 URL/Base64 图片，
并只通过各自同协议 Native Route 转发；`mimo-v2.5-pro` 和所有 Bridge 均不公开图片能力。

## 已完成内容

- Chat 接受 user message content 中的 `image_url`；Responses 接受 user input message content 中的 `input_image`。
- 两个接口允许有界 absolute HTTPS `remote_url` 与规范 Base64 `data_url`，inline MIME 集合为 JPEG、PNG、GIF、WebP、BMP。
- Responses `file_id`、显式 `detail`、非 user 位置、错误嵌套、非 HTTPS/userinfo/local 或 reserved IP literal、非法/非规范 Base64、
  未声明 MIME 和超限输入都在首次 Provider egress 前稳定拒绝。
- Provider/API 能力使用 source、MIME、detail 与 limit 组成的 typed image profile；扩展 Models 的每个接口公开由完整静态候选保守
  编译的 `multimodal_input.image`，请求 preflight 使用同一不可变 interface。
- `mimo-v2.5` 固定为两个 Native candidate，不通过请求期能力筛选、跳过或重排 Route；Native model 绑定之外不下载、转码、重排或
  修改 mixed text/image content part。

## 实现边界

- 静态 profile 位于 [`src/core/capability/generation.rs`](../../../src/core/capability/generation.rs) 与
  [`src/providers/mimo/`](../../../src/providers/mimo/)；model-specific Route surface 位于
  [`src/providers/catalog/public_models.rs`](../../../src/providers/catalog/public_models.rs)。
- 请求位置/source/MIME/detail/URL/Base64 解析位于
  [`src/pipeline/analysis/generation.rs`](../../../src/pipeline/analysis/generation.rs)，固定契约比较位于
  [`src/pipeline/preflight.rs`](../../../src/pipeline/preflight.rs)。
- 当前 OpenBridge policy 为每请求最多 64 个图片 part、单 remote URL 最多 8192 UTF-8 字节；inline profile 记录 MiMo 的 50 MB
  Base64 单图/累计上限和 checked decoded-byte ceiling。部署级 `max_request_body_bytes` 独立执行，默认 1 MiB，通常先限制大 inline
  请求。
- URL 只作为内容透传给固定 MiMo endpoint。OpenBridge 不取回媒体，因此本地检查不证明 Provider-side DNS、redirect、下载时限、
  实际 remote 文件 MIME/大小或内容安全。

## 验证证据

- `mimo_v25_image_requests_use_only_same_protocol_native_routes` 确认 Chat/Responses 图片请求各只有一个同协议 Native candidate。
- `mimo_native_image_inputs_are_preserved_for_both_protocols` 经生产 Router 与记录 transport 确认 endpoint、model、part 顺序、嵌套和
  data URL 保持，并核对扩展 Models typed image contract。
- `mimo_invalid_unsupported_and_oversized_images_fail_before_egress` 覆盖位置、嵌套、`file_id`、URL、Base64、MIME、detail、part 与 URL
  limit，并确认所有失败 case 都没有 transport request。
- `native_image_preflight_enforces_per_part_and_cumulative_inline_byte_limits` 使用窄化测试 profile 分别覆盖单图与累计 Base64/decoded-byte
  上限，确认超限在 egress 前拒绝。
- `documentation_endpoints_serve_openapi_and_swagger_ui_without_authentication` 确认运行时返回的 OpenAPI 包含 typed image contract 与
  Chat/Responses 图片请求结构。
- 2026-08-07 的真实 MiMo 直连基线使用内存 PNG data URL：Chat 与 Responses 均返回 HTTP 200、正确 object/model 和可见图片语义；
  Chat usage 还包含 image token。验证没有记录 credential、请求正文或模型原文。
- 同日以独立 Python 客户端经过临时本地 OpenBridge 实例复测：PNG data URL 与官方示例 remote URL 在 Chat/Responses 四个请求中
  都返回 HTTP 200、正确 object/model 和非空图片语义；内存红蓝图的两条结果均识别出红、蓝两色。
- 最终 checkout 基线通过 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与
  `git diff --check`；全量 Rust 测试没有失败。

确定性测试不证明真实 Provider；当次真实成功也只证明当次账号、网络、`mimo-v2.5` 和固定 PNG 请求。当前未运行 OpenAI SDK
compatibility、目标 Agent、负载、长期运行或其他媒体测试。

## 未覆盖范围

- Chat ↔ Responses 图片 Bridge、`mimo-v2.5-pro` 图片输入；
- file/audio/video、Files lifecycle、Provider-issued `file_id`、Images generation/edit；
- 媒体代理、DNS/redirect 解析、OCR、转码、内容扫描和 remote 内容取证；
- `detail`、SDK/Agent 全兼容、负载和长期运行保证。

## 相关文档

- [功能需求：Native 图片能力](../../functional-requirements/native-image.md)
- [扩展共同规则](../../functional-requirements/embedding-and-native-multimodal.md)
- [MiMo 图片协议与真实观察](../../references/providers/xiaomi-mimo-image-protocol-2026-08-07.md)
- [Chat/Responses Native 转发](native-generation-forwarding.md)
- [Models 接口与能力预检](models-api-and-capability-preflight.md)
