# 功能：`mimo-v2.5` Chat/Responses Native 图片输入

## 当前行为

- `mimo-v2.5` 的 Chat `image_url` 与 Responses `input_image` 支持受限 HTTPS URL 和规范 Base64 data URL，只走同协议
  Native Route；`mimo-v2.5-pro` 与 Bridge 不公开图片。
- Capability 使用 `RemoteUrl | DataUrl | RemoteUrlAndDataUrl` 判别联合，各 source 拥有完整 MIME/detail/limit payload；
  Provider ceiling 与 executable Target 分层，ceiling 不自动打开 Target。
- Public Model 按 source 保守相交；URL/part/encoded/decoded budget 取安全最小，MIME 取交集，data 消失时 Both 可降为 Remote-only。
- MiMo executable contract 允许 JPEG/PNG/GIF/WebP/BMP、每请求最多 64 part、URL 最多 8192 UTF-8 bytes，inline 单项/累计
  encoded 50 MiB、decoded 38 MiB，detail 只能省略。部署级 request body limit 独立生效并可能先拒绝。
- 非 user 位置、错误嵌套、`file_id`、非 HTTPS/userinfo/local/reserved IP literal、非法 Base64、MIME/detail/part/size 越界
  均在 Provider egress 前拒绝。OpenBridge 不下载、转码或扫描媒体。

## 所有权

类型/invariant 位于 `src/core/capability/generation.rs`；MiMo ceiling/Target 位于 `src/providers/mimo/`；Public Model 聚合位于
`src/registry/public_model*`；request parsing/preflight 位于 `src/pipeline/generation/analysis/image_input.rs` 与
`src/pipeline/generation/preflight.rs`。

## 确定性与真实证据

`tests/forwarding_contract.rs` 覆盖 Models 投影、compiled preflight、Chat/Responses data/remote exact egress、SSE terminal，以及
URL/Base64/MIME/detail/part/budget/file/Bridge 的 zero-egress 拒绝。

真实 MiMo 证据使用内存 PNG data URL：直连 Chat/Responses 均返回 HTTP 200 与可见图片语义；独立 Python 客户端经过临时
OpenBridge 又验证 PNG data URL 与官方示例 remote URL 的 Chat/Responses 请求，内存红蓝图被识别为红、蓝两色。没有保存
credential、请求正文、原始 Base64 或完整模型输出；解释见 [MiMo Provider 状态](../providers/mimo.md)。

## 未证明范围

真实证据只适用于当时账号、网络、`mimo-v2.5` 和固定图片。图片 Bridge、Pro 图片、`file_id`/Files、image generation/edit、
Provider-side DNS/redirect/MIME/size、OCR、内容安全、显式 detail、外部 SDK/Agent、负载和长期运行未证明。

## 相关文档

- [Native 图片需求](../../functional-requirements/extended-capabilities/native-image.md)
- [MiMo 图片参考](../../references/providers/xiaomi/image.md)
- [MiMo Provider 状态](../providers/mimo.md)
- [Models 与能力预检](models-api-and-capability-preflight.md)
