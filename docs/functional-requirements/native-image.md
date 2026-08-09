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

静态 executable profile 必须是一个完整 envelope，而不是可独立组合的 source slice 和多个 limit：

- envelope 拥有正数 `max_parts`；source 使用 `RemoteUrl(remote_limits) | DataUrl(inline_profile) |
  RemoteUrlAndDataUrl { remote, data }` 判别联合；
- Remote payload 只拥有 URL byte limit，data payload 只拥有非空、唯一的 media type set 与完整 inline encoded/decoded 单项及累计预算；
- `detail` 使用 `OmittedOnly { default } | Explicit { default, allowed }` 语义。两者都允许省略 wire 字段；只有 `Explicit`
  接受显式值且 `allowed` 必须非空、唯一。省略后的已知 default 与显式 allowed domain 是独立事实，default 不要求属于 allowed。

Public Model 按全部可执行 Route 逐 source 相交：Remote URL limit 取最小；Data URL media type 取交集并对其四项预算取保守最小。
Data media type 交集为空只移除 Data URL；若 Remote payload 仍完整则降为 Remote-only，所有 source 都消失才关闭整个图片子契约。
`max_parts`、单项预算和累计预算分别取最小后，累计 encoded/decoded 还必须以 checked wide arithmetic 收紧到
`per-item × max_parts` 的可达上限，并通过同一 checked envelope 重新验证。`detail` default 必须完全一致；任一候选为
`OmittedOnly` 时交集也是 `OmittedOnly`，全部为 `Explicit` 时 allowed 取交集，空交集安全降为保留共同 default 的
`OmittedOnly`。

扩展 Models 保持既有 flat JSON shape，但它只是上述 union 的只读投影：Remote-only 的 media type 与四项 inline limit 投影为空/`0`，
Data-only 的 URL limit 投影为 `0`，Both 投影两组正数。`0` 不是 core/registry 配置状态或 source 证据；请求 preflight 必须读取同一
编译结果中的 private owned source contract，不得反向读取 DTO。嵌套 part 字段不加入顶层 `supported_parameters`；
`modalities.input` 只作为摘要，不能替代 typed profile。

## 3. URL、Base64 与请求预检

- remote source 只接受有长度上限的 absolute HTTPS URL，拒绝 userinfo、localhost 和显式 loopback/link-local/private/reserved
  IP literal；OpenBridge 不主动下载图片或解析 redirect。
- data URL 必须使用 profile 允许的 media type 与规范 Base64，并在分配大缓冲前检查编码/解码上限。
- checked profile 的最小 wire-reachable limit 是 9 个 UTF-8 byte（`https://a`）、4 个 encoded byte（一个 Base64 quantum）和 1 个
  decoded byte。累计预算必须至少覆盖一个单项且不得超过 `per-item × max_parts`；这些只是类型可达性下界，不是对 Provider
  operational limit 的推测。
- 请求分析冻结 role、part/source、media type、detail、数量、URL 长度和 inline byte facts；非法或超限输入在首次 egress 前失败。
- Responses `file_id` 继续作为 analyzer 可识别的 wire fact，但不进入静态 source-payload union；没有 resource identity、ownership、
  affinity 与 limits 的完整 profile 时必须在首次 egress 前 fail closed。
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
| IMG-01 | Chat/Responses 分别从 source-payload union 公开 typed source、media type、detail 与 limit，并与请求 preflight 使用同一 fixed owned interface。 |
| IMG-02 | Native 上游收到原有 mixed text/image part 顺序和 wire；请求不按图片能力跳过、筛选或重排候选。                                     |
| IMG-03 | 非 user 位置、`file_id`、非法 URL/Base64/media type/detail、不可达 profile 与超限输入在 egress 前稳定拒绝。                      |
| IMG-04 | URL/data source 的确定性测试、独立客户端和真实 Provider 证据分层记录；未运行格式、尺寸、SDK、负载或长期层不声称通过。             |

## 6. 非目标与参考

非目标包括 Images API、图片生成/编辑/variation、file-backed resource、媒体下载代理、OCR、格式转换和多模态 Bridge。

- [OpenAI Chat 图片输入调研](../references/openai/images/chat-input.md)
- [OpenAI Responses 图片输入调研](../references/openai/images/responses-input.md)
- [Xiaomi MiMo 图片协议与真实观察](../references/providers/xiaomi/image.md)
