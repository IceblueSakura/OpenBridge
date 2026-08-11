# 功能：`mimo-v2.5` Chat/Responses Native 图片输入

## 状态

**已完成（当前 checkout）。** `mimo-v2.5` 的 Chat Completions 与 Responses 固定接口可以接收受限的 URL/Base64 图片，
并只通过各自同协议 Native Route 转发；`mimo-v2.5-pro` 和所有 Bridge 均不公开图片能力。

## 已完成内容

- Chat 接受 user message content 中的 `image_url`；Responses 接受 user input message content 中的 `input_image`。
- `ImageInputCapabilities` 是闭合 envelope：正数 `max_parts` 加
  `ImageSourceCapabilities::{RemoteUrl, DataUrl, RemoteUrlAndDataUrl}`。Remote variant 只拥有 URL byte limit；data variant
  只拥有非空、无重复 MIME 集合与单项/累计 encoded/decoded budget，不再使用 source slice 与无关零值组合状态。
- checked constructor 保证 Remote limit 至少容纳 `https://a` 的 9 UTF-8 bytes，inline limit 至少容纳
  `AA==` 的 4 encoded bytes/1 decoded byte；累计上限既不小于单项上限，也不得超过 `per-item × max_parts`
  的 checked `u64` 可达范围。
- detail 是 `ImageDetailPolicy::{OmittedOnly, Explicit}` 判别联合。省略 wire `detail` 后的已知 default 与客户端可显式
  提交的非空、无重复 allowed 集合是两个独立语义；MiMo 两个 Native interface 均为
  `OmittedOnly { default: None }`。
- Provider ceiling 与 executable Target 明确分层：MiMo Chat/Responses Provider ceiling 都是 Remote+Data，但只有
  `mimo-v2.5` Chat/Responses Target 保留该 profile；`mimo-v2.5-pro` 和专用 audio Target 都是 `None`。OpenAI
  Chat/Responses Provider ceiling 也是不含 FileId 的 Remote+Data，当前 checked-in OpenAI Target 仍全部为 `None`；ceiling
  不会自动打开 Target。
- Public Model 对每个 source 及其 payload 独立保守相交：URL limit 取最小，data MIME 取交集；data MIME
  空交集会关闭 data-only image，Both 则降为 remote-only。`max_parts` 与单项/累计 budget 取最小后，累计值
  再 clamp 到新的可达上限。detail default 不一致会关闭 image；同 default 下显式 allowed 空交集会降为
  `OmittedOnly`。
- 扩展 Models 保留已有平铺 JSON shape，但所有 source/media/detail/limit 都只由验证后的内部 union 总投影；
  absent source 的 `0` 只是只读 DTO 表示，不是配置状态。请求 preflight 直接读取同一 Public Model 自有的
  source-specific contract，不反向读取平铺 DTO。
- Responses `file_id` 仍是 analyzer 可识别的闭合 wire fact，但 executable image source union 不包含 FileId；在尚无
  resource identity/ownership/affinity/limit 契约时稳定 fail closed，且不出现在 Models DTO。Bridge 也不贡献任何
  image source，因此 Native image 请求不会通过跨协议 Route。
- 两个 MiMo Native 接口允许有界 absolute HTTPS `remote_url` 与规范 Base64 `data_url`，inline MIME 集合为
  JPEG、PNG、GIF、WebP、BMP。显式 `detail`、非 user 位置、错误嵌套、非 HTTPS/userinfo/local 或 reserved IP
  literal、非法/非规范 Base64、未声明 MIME 和超限输入都在首次 Provider egress 前稳定拒绝。
- `mimo-v2.5` 固定为两个 Native candidate，不通过请求期能力筛选、跳过或重排 Route；Native model 绑定之外不下载、转码、重排或
  修改 mixed text/image content part。

## 实现边界

- core union 与 checked invariant 位于 [`src/core/capability/generation.rs`](../../../src/core/capability/generation.rs)；MiMo
  Provider ceiling 与 Target narrowing 分别位于 [`src/providers/mimo/definition.rs`](../../../src/providers/mimo/definition.rs) 与
  [`src/providers/mimo/registration.rs`](../../../src/providers/mimo/registration.rs)，model-specific Route surface 位于
  [`src/providers/catalog/public_models.rs`](../../../src/providers/catalog/public_models.rs)。
- Public Model 自有 union、交集、checked narrowing 和平铺 DTO 投影位于
  [`src/registry/public_model.rs`](../../../src/registry/public_model.rs)；compiler 只将每个可执行候选的 image contract 送入该交集。
- 请求位置/source/MIME/detail/URL/Base64 解析位于
  [`src/pipeline/analysis/generation/image_input.rs`](../../../src/pipeline/analysis/generation/image_input.rs)，编译后固定契约比较位于
  [`src/pipeline/preflight.rs`](../../../src/pipeline/preflight.rs)。
- MiMo executable profile 为每请求最多 64 个图片 part、单 remote URL 最多 8192 UTF-8 bytes；inline profile 为
  50 MiB Base64 encoded 单图/累计上限和 38 MiB decoded 单图/累计上限。部署级 `max_request_body_bytes`
  独立执行，默认 1 MiB，通常先限制大 inline 请求。
- URL 只作为内容透传给固定 MiMo endpoint。OpenBridge 不取回媒体，因此本地检查不证明 Provider-side DNS、redirect、下载时限、
  实际 remote 文件 MIME/大小或内容安全。

## 验证证据

- `image_input_capabilities_bind_each_source_to_its_complete_payload` 固定三个 source variant、source-specific getter、detail
  两个 variant 与 default/allowed 独立语义；core 负例固定 URL 8 bytes、inline encoded 3 bytes、空/重复集合、
  `max_parts = 0` 及不可达累计 budget 在 profile 构造边界被拒绝。
- `provider_image_ceiling_accepts_each_source_subset_and_rejects_payload_elevation` 和
  `image_provider_ceilings_and_checked_in_targets_keep_separate_source_evidence` 固定 source subset lattice、同源 payload narrowing、
  MiMo/OpenAI Chat/Responses Provider ceiling 以及当前 Target 的独立收窄，并逐项核对四个 inline limit。
- `mimo_v25_image_requests_use_only_same_protocol_native_routes` 确认 Chat/Responses 图片请求各只有一个同协议 Native candidate。
- `native_image_preflight_accepts_the_minimum_legal_remote_and_data_wires` 证明最小合法 `https://a` 与
  `data:image/png;base64,AA==` 经同一 compiled preflight 通过。
- `public_image_projection_preserves_exact_source_specific_payloads`、
  `public_image_intersection_closes_or_downgrades_disjoint_data_sources`、
  `public_image_detail_intersection_keeps_omission_separate_from_explicit_values` 与
  `public_image_intersection_clamps_cross_minima_to_reachable_inline_totals` 分别固定 Remote/Data/Both 精确 JSON 投影、
  MIME 空交集关闭/降级、detail default/allowed 矩阵、cross-minima clamp 及聚合后 preflight 边界。
- `mimo_native_image_inputs_are_preserved_for_both_protocols` 经生产 Router 与记录 transport 对比四个完整原始 request
  body：Chat/Responses 各覆盖 data URL + JSON 与 remote URL + `stream: true`，从而同时固定 endpoint、model、role、
  顶层 control field、mixed part 顺序和协议嵌套；两条 SSE 还精确比对 wire 并验证完整 terminal lifecycle。
- `mimo_invalid_unsupported_and_oversized_images_fail_before_egress` 覆盖位置、嵌套、`file_id`、URL、Base64、MIME、detail、part 与 URL
  limit，并确认所有失败 case 都没有 transport request。
- `native_image_preflight_enforces_per_part_and_cumulative_inline_byte_limits` 使用窄化测试 profile 分别覆盖单图与累计 Base64/decoded-byte
  上限，确认超限在 egress 前拒绝。
- `production_router_rejects_unbridgeable_requests_before_egress` 确认 Bridge 不公开 image source，Native image part 在
  credential/transport 之前被拒绝。
- `documentation_endpoints_serve_openapi_and_swagger_ui_without_authentication` 确认运行时返回的 OpenAPI 包含 typed image contract 与
  Chat/Responses 图片请求结构。
- 2026-08-07 的真实 MiMo 直连基线使用内存 PNG data URL：Chat 与 Responses 均返回 HTTP 200、正确 object/model 和可见图片语义；
  Chat usage 还包含 image token。验证没有记录 credential、请求正文或模型原文。
- 同日以独立 Python 客户端经过临时本地 OpenBridge 实例复测：PNG data URL 与官方示例 remote URL 在 Chat/Responses 四个请求中
  都返回 HTTP 200、正确 object/model 和非空图片语义；内存红蓝图的两条结果均识别出红、蓝两色。

上述确定性 Rust 证据不证明真实 Provider；当次历史真实成功也只证明当次账号、网络、`mimo-v2.5` 和固定 PNG
请求。本轮 typed image profile 收口没有重跑真实 Provider、OpenAI SDK compatibility、目标 Agent、负载、长期运行或其他媒体测试。

## 未覆盖范围

- Chat ↔ Responses 图片 Bridge、`mimo-v2.5-pro` 图片输入；
- file/audio/video、Files lifecycle、Provider-issued `file_id`、Images generation/edit；
- 媒体代理、DNS/redirect 解析、OCR、转码、内容扫描和 remote 内容取证；
- MiMo 显式 `detail`、SDK/Agent 全兼容、负载和长期运行保证。

## 相关文档

- [功能需求：Native 图片能力](../../functional-requirements/extended-capabilities/native-image.md)
- [扩展共同规则](../../functional-requirements/extended-capabilities/embedding-and-native-multimodal.md)
- [MiMo 图片协议与真实观察](../../references/providers/xiaomi/image.md)
- [MiMo Provider 多模态与工具调用状态](../providers/mimo.md)
- [Chat/Responses Native 转发](native-generation-forwarding.md)
- [Models 接口与能力预检](models-api-and-capability-preflight.md)
