# Chat/Responses JSON 多模态实现细节

**目标状态：** 已批准但尚未进入当前开发焦点；本文只保留待实现的协议约束，不构成实施授权。当前源码仍会把 Chat audio/file 与 Responses file 请求归入 reserved/unimplemented 拒绝路径。

## 1. 范围与协议差异

本目标只扩展现有 `POST /v1/chat/completions` 与 `POST /v1/responses` 的 Native JSON/SSE 路径，使已注册接口可以显式承载图像、内联/URL 文件与 Chat 输入音频 part。它不包含 Chat audio output、专用 `/audio/*`、`/images/*`、Files lifecycle、file search 或 Realtime。

Chat 与 Responses 必须拥有不同 parser 和 capability；逻辑模态相同不表示 wire 可互换：

| 语义 | Chat Completions | Responses |
|---|---|---|
| 文本 | string 或 user `type: "text"` content part | 顶层 string、message item 中的 `input_text` |
| 图像 | user `type: "image_url"`，`image_url.url` 承载远程 URL/data URL | `type: "input_image"`，一选一 `image_url` 或 `file_id` |
| 文件 | user `type: "file"`，嵌套 `file.file_data` 或 `file.file_id`，可带 `filename` | `type: "input_file"`，一选一 `file_data`、`file_url` 或 `file_id`，可带 `filename` |
| 音频输入 | user `type: "input_audio"`，嵌套 base64 `data` 与 `format` | 本阶段无标准输入形状 |
| 输入 detail | image 的 `image_url.detail` | image 的顶层 `detail`；file 的顶层 `detail` 只适用于已声明文件 profile |
| 音频输出 | 顶层 `modalities`/`audio` 与 response audio | 本阶段不实现，也不从 Chat 自动映射 |

官方资料：[Chat Create](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create)、[Responses Create](https://developers.openai.com/api/reference/resources/responses/methods/create)、[Images and vision](https://developers.openai.com/api/docs/guides/images-vision)、[File inputs](https://developers.openai.com/api/docs/guides/file-inputs)、[Audio and speech](https://developers.openai.com/api/docs/guides/audio)及[官方 Python SDK Chat content union](https://github.com/openai/openai-python/blob/main/src/openai/types/chat/chat_completion_content_part_param.py)。

## 2. 第一版逐协议 source contract

### 2.1 Chat `image_url`

标准形状位于 `role: "user"` 的 content array：

```json
{
  "type": "image_url",
  "image_url": {
    "url": "https://example.invalid/image.png",
    "detail": "auto"
  }
}
```

`url` 必填且归类为：

- `remote_url`：有界绝对 HTTPS URL；
- `data_url`：`data:<allowed-image-media-type>;base64,<payload>`；
- 其他 scheme、相对 URL、裸 base64 或无法确定的 data URL：拒绝。

OpenAI Images guide 当前称省略 `detail` 时默认为 `auto`，显式值可为 `auto/low/high/original`，且 Chat/Responses 行为相同；但当前官方 Python SDK 的生成 Chat 类型仍只列 `auto/low/high`，Responses 类型已经列出 `original`。因此 OpenBridge 不维护跨协议全局枚举：每个 Upstream API 必须分别声明 default 与 allowed set，OpenAI SDK 验收应在实施时再次确认 `original` 的实际兼容状态。

### 2.2 Responses `input_image`

标准形状：

```json
{
  "type": "input_image",
  "image_url": "data:image/png;base64,...",
  "detail": "high"
}
```

`image_url` 可为 `remote_url` 或 `data_url`；官方 schema 另允许 `file_id`。必须恰好提供一个 source。第一版 `file_id` 在请求分析阶段稳定拒绝，不能尝试把它解释为 URL 或投向多个 Target。

### 2.3 Chat `file`

当前官方生成 SDK 的 user-message content union 使用嵌套对象：

```json
{
  "type": "file",
  "file": {
    "filename": "notes.txt",
    "file_data": "<base64>"
  }
}
```

该 schema 只把 `file_data` 描述为 base64 string，并允许另选 `file_id`；它没有 `file_url`、显式 MIME 字段或 file `detail`。第一版只允许 `inline_data + raw_base64`，要求非空且有界的 `filename`，并拒绝 `file_id`、缺失 source、多个 source 或把 URL/MIME 填入未知字段。若某 Provider 实测要求 data URL，必须由独立 profile 声明 `data_url` encoding，不能改变 Chat 全局语义。

### 2.4 Responses `input_file`

Responses 可使用：

- `inline_data`：`file_data: "data:<media-type>;base64,..."`，并提供 filename；
- `remote_url`：`file_url: "https://..."`；
- `file_id`：由 Files API 签发，第一版拒绝。

三种字段必须恰好出现一个。`detail` 只接受该 file profile 声明的值；官方当前对 PDF 列出 `auto/low/high`，省略时为 `auto`，并明确 Chat file 不支持该字段。不能把 PDF detail 自动应用于任意文档或 Chat。

### 2.5 Chat `input_audio`

标准形状位于 `role: "user"` 的 content array：

```json
{
  "type": "input_audio",
  "input_audio": {
    "data": "<base64>",
    "format": "wav"
  }
}
```

`data` 是裸 base64，而不是远程 URL；当前官方 Chat 类型列出 `wav` 与 `mp3`。第一版要求 format 属于接口集合、payload 非空且编码/解码大小有界。该能力不打开顶层 audio output，也不推断 Responses audio input、transcription 或 Realtime。

## 3. 类型化 capability 模型

现有 `image_input`/`file_input`/`audio_input` 布尔值无法保护 source、detail、format 与大小边界。Chat/Responses 的 `ModelInterfaceCapabilities` 应增加协议内的 `multimodal_input` 投影，并使用不同的子类型：

| 子契约 | 必填能力字段 |
|---|---|
| `image` | `sources`、`inline_media_types`、`detail.default`/`detail.allowed`、`max_parts`、`max_remote_url_bytes`、`max_inline_encoded_bytes_per_part`、`max_inline_decoded_bytes_per_part` |
| `file` | `sources`、`inline_encodings`、可选 `inline_media_types`、filename 规则、`detail.default`/`detail.allowed`/适用类别，以及同类 URL/inline/part limits |
| `audio` | `sources`、`formats`、part 数及 inline 编码/解码字节 limits |
| `multimodal_input.limits` | `max_media_parts`、`max_total_inline_encoded_bytes`、`max_total_inline_decoded_bytes` |

约束：

- `sources` 使用闭合枚举 `remote_url`、`data_url`、`inline_data`、`inline_base64`、`file_id`；parser 负责从具体协议 wire 映射，业务请求不能直接提交 enum 名称。
- file `inline_encodings` 至少区分 `raw_base64` 与 `data_url`；只有 data URL wire 才能从 payload 验证显式 media type，raw base64 不能根据 filename extension 伪造 MIME 证据。
- `detail.default` 描述省略字段时的稳定行为，`detail.allowed` 描述可显式发送的 profile 集合；default 不同的候选不能共享该媒体子契约。
- `formats` 与 media type 都是 profile 集合，不存在全 Provider 默认集合。
- image/file/audio 子契约缺失即 unsupported；所有公开集合必须非空、去重并确定排序。
- `modalities.input` 继续提供 text/image/audio/file 摘要，但只有对应子契约才能证明某种具体请求可执行。
- 嵌套 part/source/detail/format 不加入顶层 `supported_parameters`；后者仍只表示 endpoint 顶层可选字段。
- Public DTO 不公开 URL allowlist、Provider、Target、Route、credential、file issuer 或内部 policy implementation。

## 4. 能力聚合规则

每个 Public Model/下游协议只编译一份与固定候选绑定的 execution interface：

1. 对所有静态可执行 candidate 分别生成协议能力贡献。
2. Native candidate 贡献其 Upstream API 明确声明的 source/format/detail/media type 与 limits。
3. Bridged candidate 对本阶段 image/file/audio 贡献空集，因为 Bridge 尚无无损转换证明。
4. 枚举集合取交集、数字上限取最小值；显式 detail 交集为空但 default 一致时令 allowed 为 `null`，default 不一致或必需集合为空时不公开该子契约。
5. canonical model modality 缺失或不包含该 modality 时不能提升；包含也只能作为上界继续与 API contract 相交。
6. 预检使用的 capability 实例与扩展 Models 序列化的实例必须来自同一个 execution interface，禁止重复计算。

这一规则意味着：若一个 Public Model 的固定 Chat 候选包含较强 Native Route 和不支持媒体的 Chat←Responses Bridge fallback，则该 Chat interface 不能公开多模态能力。正确做法是调整静态 Route surface 或完成 Bridge contract，而不是请求时跳过 Bridge。

## 5. 请求分析与 fail-closed 预检

请求分析必须在 Route planning 前一次性遍历协议允许的位置，并冻结：

- part 的协议类型与原始顺序；
- image/file/audio 的 source kind；
- inline encoding、detail、format、可验证的 media type 与 filename 是否存在；
- 每类 part 数、总 media part 数；
- remote URL 长度，以及 data URL/raw base64 的 inline 编码长度与安全解码后的字节数；
- 是否出现 source 冲突、非法 base64、未知 detail/format 或第一版禁止的 `file_id`。

解析规则应使用精确 path，不递归扫描任意同名字段：

- Chat 只把 `messages[*].role == "user"` 的标准 content union 识别为 image/file/audio；这些 part 出现在 developer/system/tool/assistant 等不允许角色时按 malformed request 拒绝；
- Responses 检查标准 input item 及 message `content[*]`；
- tool 参数、metadata 或用户文本中的 `type`/`file_id` 字符串不能被误判为媒体 part。

preflight 将 frozen requirements 与一个固定 interface 比较；任何 source/format/detail/media type/limit 不满足都在首次 egress 前返回稳定 400。通过后 planning 不再读取媒体能力，也不筛选、跳过或重排候选。

## 6. Native 转发与响应

Native adapter 应保留合法 part 的对象层级、顺序、URL/data、detail、filename、format 和当前 profile 已允许的字段；只改写受信 model、path、auth/header。OpenBridge 第一版不主动 fetch、落盘、转码、OCR、转写、压缩或把 file/image/audio 替换成文本。

输入多模态不改变原 endpoint 的响应状态机：

- Chat JSON/SSE 仍使用 Chat completion/chunk 与 `[DONE]` 终态；
- Responses JSON/typed SSE 仍使用 Responses item/event 与 `response.completed` 等终态；
- 首个下游业务输出后不 fallback，不拼接另一个 Target 的结果；
- 下游取消停止当前 body/stream 和待执行 backoff。

Bridge 第一版保持 fail closed：不把 media 转文本、不把 file ID 当 URL、不下载后偷偷改成 data URL，也不把 audio output 降级成 transcript。

## 7. URL、base64 与日志安全

### 7.1 Remote URL

第一版 OpenBridge 只执行可确定的入站预检：

- 绝对 `https` URL；有最大字符/字节长度；
- 禁止 userinfo、空 host、`localhost` 及显式 loopback/link-local/private/reserved IP literal；
- URL 只能保留在业务 body，不能影响 upstream origin、Host、Authorization、proxy、credential 或 Route；
- 不在网关 DNS resolve、follow redirect 或下载内容。

因为实际 fetch 由 Provider 执行，网关不能证明 Provider 侧 DNS rebinding、redirect、内容类型、下载时限或最大响应字节。真实 Provider 测试必须明确这一证据缺口，文档不能把语法预检表述为完整 SSRF 防护。

### 7.2 Inline data

- 在 base64 decode 前先验证 JSON/data URL 长度和预计解码上界，避免不受控分配。
- data URL 必须是 `data:<declared-media-type>;base64,<payload>`；Chat file/audio 的目标 profile 可以按协议要求裸 base64。
- 严格解码并同时执行 per-part、per-modality 与 request-total 限制；失败信息不回显 payload。
- MIME/format 必须属于 interface 集合；filename extension 不是唯一的格式证据。
- 普通日志、trace、metrics label 和错误上下文不得包含 URL query、filename、file ID、data URL 或媒体 bytes。

## 8. TDD 与验收矩阵

| case | 必须证明的行为 |
|---|---|
| capability compilation | source/format/detail allowed/media type 求交、detail default 一致、limit 取小、Bridge 空贡献、Models 与 preflight 同源 |
| Chat image | remote/data URL 分类、detail gate、mixed text/image 顺序与 Native wire |
| Responses image | remote/data URL；多个 source 与 `file_id` 在 egress 前拒绝 |
| Chat file | 只接受嵌套 raw-base64 inline data + filename；URL/MIME/detail/ID 或 source 冲突拒绝 |
| Responses file | inline/remote URL、filename/detail 规则；ID/source 冲突/超限拒绝 |
| Chat input audio | `wav`/`mp3` profile、严格 base64、编码/解码 limit；只走 Chat Native |
| 固定候选 | 不因媒体请求跳过、筛选或重排 Route；弱候选会在编译期收窄公共契约 |
| stream/cancel | Chat/Responses 各自 terminal、首输出 commit、取消和 retry boundary 不变 |
| security | HTTPS 语法 policy、非业务控制面、日志脱敏与错误不回显 payload |
| client evidence | 独立 Python/OpenAI SDK 或 curl 对 loopback 的协议请求；真实 Provider 另行授权与记录 |

确定性 fixture 使用小型合成 PNG/PDF/text/WAV，不包含私人文件或真实 URL token。若修改 canonical `testdata/` 或 `tools/corpus/`，必须同时运行 Python corpus baseline；Rust replay 与 loopback 只证明其直接覆盖层。

## 9. 非目标

- 不实现媒体下载代理、OCR、转写、格式转换、压缩或通用内容审核；
- 不实现 audio output、Files 资源生命周期、file search、image generation 或 Realtime；
- 不接受 Provider-issued `file_id`，不建立 resource ledger 或跨 Target 资源迁移；
- 不承诺 Chat ↔ Responses 多模态 Bridge；
- 不因 canonical modality、Native passthrough 或首选 Route 能力较强就扩大固定接口；
- 不允许业务请求选择 upstream URL、credential、header transform、Provider 或 Route。
