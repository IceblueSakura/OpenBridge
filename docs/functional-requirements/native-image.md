# Native 图片能力需求

## 范围

本页只定义 Chat `image_url` 与 Responses `input_image` 的同协议 Native 输入能力。它不定义 Images generation/edit/variation、
文件、音频、视频或跨协议媒体转换；共同规则见[媒体扩展共同规则](embedding-and-native-multimodal.md)。当前已完成切片与实际证据见
[`mimo-v2.5` Native 图片输入](../implementation-status/features/native-image-input.md)。

## 1. 用户结果与 wire

| 协议 part               | 可建模来源                          | 固定边界                                                                  |
|-------------------------|-------------------------------------|---------------------------------------------------------------------------|
| Chat `image_url`        | `remote_url`、`data_url`            | 只在 user message content 中有效；省略/显式 `detail` 分别服从 profile     |
| Responses `input_image` | `remote_url`、`data_url`、`file_id` | 当前目标只开放 URL/data URL；没有 resource affinity 时必须拒绝 `file_id` |

图片 part 必须出现在协议规定的 user content union 中；developer/system/tool/assistant 或任意递归同名字段都不能被当作合法图片输入。
Native 转发保持 mixed text/image part 的顺序、类型、URL/data、detail 与原协议 JSON/SSE terminal，只允许受信 model/path/auth/header
改写及 Public Model response projection。

## 2. `multimodal_input.image` 公共契约

每个 Chat/Responses interface 的图片子契约必须明确：

- 允许的 `remote_url`/`data_url` source；
- data URL media type 集合；
- `detail` default 与 allowed domain；
- 单请求图片 part 数、单 URL UTF-8 长度；
- 单项和累计 inline encoded/decoded byte 上限。

子契约缺失、source/media type 交集为空或 detail default 不一致即表示该接口不支持图片。嵌套 part 字段不加入顶层
`supported_parameters`；`modalities.input` 只作为摘要，不能替代 typed profile。

## 3. URL、Base64 与请求预检

- remote source 只接受有长度上限的 absolute HTTPS URL，拒绝 userinfo、localhost 和显式 loopback/link-local/private/reserved
  IP literal；OpenBridge 不主动下载图片或解析 redirect。
- data URL 必须使用 profile 允许的 media type 与规范 Base64，并在分配大缓冲前检查编码/解码上限。
- 请求分析冻结 role、part/source、media type、detail、数量、URL 长度和 inline byte facts；非法或超限输入在首次 egress 前失败。
- Provider fetch 的 DNS、redirect、下载时限、远端 MIME/大小和内容安全属于真实 Provider 边界，入站 URL 检查不能冒充完整 SSRF
  防护。

## 4. Route、Bridge 与数据保护

- 公共能力是同一 interface 全部静态可执行 Route 的保守交集；图片请求通过 preflight 后仍保留完整固定候选顺序。
- Bridged Route 对图片 source 贡献空集；图片请求不得通过 Chat ↔ Responses Bridge，也不得按请求跳过较弱 Route。
- 网关不得下载、转码、OCR、重排、缓存或把图片替换成文本。
- URL query、原始图片、Base64、完整响应和解码错误上下文不得进入普通日志或 metrics label。
- 大 body 超过 replay budget 时只执行第一次 attempt；首个下游业务输出后不得 retry/fallback。

## 5. 验收

| ID     | 应被保护的可观察行为                                                                                                               |
|--------|------------------------------------------------------------------------------------------------------------------------------------|
| IMG-01 | Chat/Responses 分别公开 typed source、media type、detail 与 limit，并与请求 preflight 使用同一固定 interface。                     |
| IMG-02 | Native 上游收到原有 mixed text/image part 顺序和 wire；请求不按图片能力跳过、筛选或重排候选。                                     |
| IMG-03 | 非 user 位置、`file_id`、非法 URL/Base64/media type/detail 与超限输入在 egress 前稳定拒绝。                                      |
| IMG-04 | URL/data source 的确定性测试、独立客户端和真实 Provider 证据分层记录；未运行格式、尺寸、SDK、负载或长期层不声称通过。             |

## 6. 非目标与参考

非目标包括 Images API、图片生成/编辑/variation、file-backed resource、媒体下载代理、OCR、格式转换和多模态 Bridge。

- [OpenAI Chat 图片输入调研](../references/openai/images/chat-input.md)
- [OpenAI Responses 图片输入调研](../references/openai/images/responses-input.md)
- [Xiaomi MiMo 图片协议与真实观察](../references/providers/xiaomi/image.md)
