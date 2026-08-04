# Chat/Responses JSON 多模态实现细节

**目标状态：** 已批准的现阶段目标；等待 Embeddings 当前焦点收口后，才能另立单一焦点实施。

## 范围

本协议族只扩展现有 `POST /v1/chat/completions` 与 `POST /v1/responses` 的 Native JSON/SSE 路径，使已注册接口可以显式承载图像、内联/URL 文件与 Chat 输入音频 part。现阶段目标只覆盖输入，不包含 Chat audio output、专用 `/audio/*`、`/images/*`、Files lifecycle、file search 或 Realtime。

Chat 与 Responses 的 content schema 不同：

| 语义 | Chat Completions | Responses |
|---|---|---|
| 文本 | string 或 `text` content part | string、message item 中的 `input_text` |
| 图像 | `image_url` part，承载 URL/data URL 与 detail | `input_image`，承载 `image_url` 或 `file_id` |
| 文件 | `file` content part，按当前 schema 校准 file data/id | `input_file`，可承载 `file_data`、`file_url` 或 `file_id` |
| 音频输入 | `input_audio`，含 base64 data 与 format | 当前 Responses profile 不从 Chat 字段类推支持 |
| 音频输出 | `modalities` 包含 audio，并提供 `audio` 配置 | 本阶段不实现，也不从 Chat 自动映射到 Responses |

官方资料：[Chat API](https://developers.openai.com/api/reference/resources/chat)、[Create response](https://developers.openai.com/api/reference/resources/responses/methods/create)、[Images and vision](https://developers.openai.com/api/docs/guides/images-vision)、[File inputs](https://developers.openai.com/api/docs/guides/file-inputs) 与 [Audio and speech](https://developers.openai.com/api/docs/guides/audio)。

## 第一版输入边界

为了在没有通用 resource ledger 的情况下保持可证明性，建议第一版按来源分层：

| 来源 | 第一版策略 | 原因 |
|---|---|---|
| inline text | 放行 | 已有能力 |
| image URL | 仅在接口声明支持时 Native 保留 | 上游会执行外部 fetch，需要 URL policy |
| image data URL | 仅在编码/解码大小均有界时 Native 保留 | 不能泄露到日志，存在 base64 膨胀 |
| inline `file_data` | 仅在协议/profile 明确支持且有界时 Native 保留 | 无 issuer，但媒体和 filename 仍需校验 |
| external `file_url` | 独立 URL policy 后 Native 保留 | 上游 fetch 带来 SSRF/重定向风险 |
| upstream `file_id` | 第一版拒绝 | 当前没有 issuer/Target ledger，fallback 不安全 |
| Chat `input_audio` | 仅目标 Chat Native profile 支持时放行 | 不进入 Responses Bridge |
| Chat audio output | 本阶段拒绝 | 输入多模态不能扩大输出 modality |

如果产品选择同时实现 Files 并先引入网关 opaque resource ID，则可把 `file_id` 解码为固定 issuer 后放行；在此之前不得把任意 ID 尝试到多个 Target。

## capability 与请求分析

当前 capability 已预留 audio/file 字段，但请求分析会统一返回 `UnimplementedCapabilities`。正式实现应把“预留”替换为逐协议、逐来源的可执行能力，而不是一个 `multimodal=true`：

- Chat: image URL/data、input audio formats和 file forms；audio output 保持未实现；
- Responses: input image URL/data、input file data/URL/ID、detail 等；
- 共同限制：part count、单 part 编码字节、解码字节、总媒体字节、URL policy；
- Bridgeability: 每个 part 是否可无损映射，默认 false。

Public Model 的接口能力仍由所有可执行 Route 保守相交。请求先对固定接口预检，不能为了一个 image/file part 临时筛选或重排 Route。

## Native、Bridge 与响应

Native 路径应保留合法未知字段、content part 顺序、URL/data、detail、filename、audio format 和 SSE 原始 bytes；只改写受信 model/header/path。它仍必须验证所选 Provider profile 的 endpoint 和 capability。

Bridge 第一版保持 fail closed：

- 不把 image、audio 或 file 内容转成占位文本；
- 不把 `file_id` 当作 URL，也不下载后偷偷改成 data URL；
- 不把 Chat audio output 降级为 transcript；
- 不因两个 part 名称相似就认定 Chat/Responses schema 等价；
- 任一 part、annotation、output item 或 stream event 无法表示时，必须在出站前拒绝。

未来的 Chat audio output 可能把 base64 audio 放入 JSON/SSE message；它与 `/audio/speech` 的 raw binary response 不共用 terminal，但不属于现阶段目标。Responses 的 typed SSE 继续使用 Responses lifecycle，不因输入是 image/file 而改变终态规则。

## URL、媒体与日志安全

- 远程 URL 是业务 payload，不得拼接为 OpenBridge upstream base URL，也不得影响 Host、Authorization、proxy 或 redirect policy。
- URL policy 至少要覆盖 scheme、credential-in-URL、私网/loopback、DNS rebinding、redirect、下载时限和最大字节；若网关不执行 fetch，也要说明上游可能执行 fetch 的风险。
- data URL/base64 同时限制编码后 JSON、解码后媒体和累计媒体大小；校验 MIME/format，但不依赖文件扩展名作为唯一证据。
- request/error/trace 不保存原始媒体、URL query、filename 或 file ID；这些值不得成为 metrics label。
- 下游取消必须停止当前上游 body/stream；首个业务输出后不再 fallback。

## TDD 与验收矩阵

| case | 预期 |
|---|---|
| text + image URL/data URL 顺序 | Native 上游与下游 JSON 顺序一致，仅 model/auth 被受信改写 |
| Responses `input_file` data/URL | 已声明 profile 放行，未声明或超限在 egress 前失败 |
| 任意 `file_id`（无 ledger） | 稳定拒绝，不尝试多个 Target |
| Chat `input_audio` | format 和 base64 有界；只走 Chat Native |
| Chat audio output JSON/SSE | 本阶段在 egress 前稳定拒绝 |
| Bridge candidate | 含任一未建模媒体 part 时在第一次上游调用前失败 |
| mixed candidates | 固定 Public Model contract 预检，不按请求临时挑“支持图片”的 Route |
| cancellation/partial stream | 首输出后不 fallback，不拼接两个上游的媒体或文本 |

确定性 corpus 应包含小型合成 PNG、PDF/data URL 和 WAV base64，不包含私人文件。外部 SDK case 与真实 Provider 只在实现完成后作为额外证据，且要分别记录实际 model、endpoint、字段和响应形状。

## 非目标

- 不实现媒体下载代理、OCR、转写、格式转换、压缩或内容审核服务；
- 不在本焦点实现 audio output、Files 资源生命周期、file search、image generation 或 Realtime；
- 不承诺 Chat ↔ Responses 多模态 Bridge；
- 不因 canonical Model 声明 image/audio/file modality 就自动扩大接口能力；
- 不接受业务请求选择 upstream URL、credential、header transform 或 Route。
