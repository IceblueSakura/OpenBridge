# Images 协议实现细节

**目标状态：** 仅作协议参考；已从现阶段实施目标移除。

## 范围与 endpoint

本协议族覆盖专用 Image API：

| operation | OpenAI path | 当前主要请求/响应形状 |
|---|---|---|
| generation | `POST /v1/images/generations` | JSON；JSON 或 image generation SSE |
| edit | `POST /v1/images/edits` | 当前 endpoint schema 以 JSON image reference 为主；JSON 或 SSE |
| variation | `POST /v1/images/variations` | multipart image；JSON 成功响应，且当前只适用于特定旧模型 |

官方 guide/SDK 还展示 file-backed image 输入。实施时必须再次对照目标 Provider 的最新 endpoint schema，明确是 JSON `image_url`/`file_id`、multipart file，还是两者都支持；不能用 SDK helper 的入参类型猜测 wire。官方资料：[Image generation](https://developers.openai.com/api/docs/guides/image-generation)、[Images API](https://developers.openai.com/api/reference/resources/images) 与 [Create image variation](https://developers.openai.com/api/reference/resources/images/methods/create_variation)。

本协议不包含 Chat/Responses 的 image input，也不把 Responses `image_generation` hosted tool 自动视为 Images API。

## operation capability

Images 需要独立于文本 generation 的 capability：

- operation: generation/edit/variation；
- model mapping 和每个 model 允许的 operation；
- request media types 和 image reference forms；
- size、quality、background、output format、compression、moderation 等字段；
- edit 的 image count、mask、input fidelity；
- stream 与 partial image 数；
- URL/base64 output forms、响应/event byte limits；
- input/output resource affinity 与 retry safety。

不能把 `OutputModality::Image` 当作所有上述行为的证明。Public Model 的 `images` interface 应公开可直接用于请求预检的 parameter/format contract，仍隐藏真实 Provider/Target/model。

## ingress 与 Native 转发

JSON operation 走有界 JSON parser；multipart operation 走独立 multipart ingress。二者共享认证、受信 model/path/auth 和观测，但不共享 body parser。

Native 转发应：

1. 校验 Public Model、operation、content type、model-specific 参数和输入引用；
2. 保留 prompt 与 image reference 顺序，只改写真实 upstream model；
3. 对 URL/data URL/file ID 分别执行 policy 和 affinity；
4. 根据请求 mode 选择 JSON 或 Images SSE response state；
5. 原样保留合法 base64/URL/partial-image payload，不解码、重采样或上传到另一个存储；
6. 对响应大小、event size、terminal、错误和取消进行有界处理。

若第一版没有 resource ledger，edit 中的 upstream `file_id` 必须拒绝；可以先支持 inline/data URL 或 multipart 文件，但不能在候选失败后把 ID 试到另一 Target。

## response、URL 与 streaming

JSON 成功通常包含生成时间与 `data[]`，图片可能通过 `b64_json` 或 URL 表达。网关不得把 URL 下载后改成 base64，也不得上传 base64 后改成 URL；两种形式在延迟、TTL、数据暴露和失败边界上不同。

Images SSE 是独立事件族。partial image 可能多次携带较大的 base64 payload；必须同时限制单事件、累计响应与 partial count。首个 partial event 下发后不能 fallback，不能把两个 generation 拼成一个流，也不能借用 Responses `response.completed` 规则推断终态。

返回 URL 可能有短 TTL 或签名 query。不得记录完整 URL、把它作为 metrics label、修改 query，或承诺 OpenBridge 保管其长期可用性。

## retry 与副作用

图像生成/编辑会产生费用和随机结果。仅在明确可重放、尚未提交下游业务输出、attempt budget 允许且目标契约等价时考虑 retry。timeout 不证明上游没有创建结果；默认不跨 Provider 盲目 fallback。variation/edit 的上传 body 还必须满足 replay byte budget。

若未来支持 idempotency key，必须先确认目标 Provider 的真实语义与 header allowlist，不能由网关自创字段假装幂等。

## 安全与观测

- prompt、input image、mask、base64、file ID 与完整输出 URL 不写日志。
- URL 输入应用外部 fetch policy；data URL 同时限制编码和解码大小；multipart 限制 boundary、part、filename 和总字节。
- image model/content policy 的上游拒绝按安全低基数错误返回，不回显内部 moderation 诊断或完整业务内容。
- 请求不能控制 upstream base URL、credential、Host、Authorization、proxy 或 header transform。

## TDD 与验收矩阵

| case | 必须证明 |
|---|---|
| generation JSON | model rewrite、参数 gate、base64/URL 响应保持、usage/metadata 不丢失 |
| generation SSE | partial event 分片、累计 limit、唯一 terminal、EOF/取消、首输出后不 fallback |
| edit | image references/mask/count/fidelity 与目标 profile 一致，未知 issuer ID 拒绝 |
| variation multipart | boundary、filename、media、model rewrite、只允许声明的 model/format |
| mixed candidates | 固定接口 contract，不为某个参数临时筛选 Route |
| errors | 非成功 JSON、错误 media type、超限 base64/event 和上游断流不伪装成功 |

canonical corpus 只使用小型合成图片和 mask。真实 Provider 验收应记录具体 model、operation、请求 media type、stream、output form 和实际响应，不以视觉主观质量代替 wire contract。

## 非目标

- 不实现 Responses image-generation hosted tool 或 Chat/Responses image input；
- 不做图片格式转换、缩放、压缩、内容缓存、URL 托管或 CDN；
- 不跨 Provider 转换 prompt/model 风格或保证随机结果等价；
- 不因 model 输出 modality 为 image 就自动开放全部 Images operation；
- 不在同一焦点实现 Files、Videos 或 Realtime。
