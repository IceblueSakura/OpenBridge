# OpenAI 自定义声音与 consent 调研

## 来源、范围与快照

本文只记录自定义 voice 前置 consent 与 voice create resource。Speech 合成、音频转写、translation、Chat audio 与 Realtime 不在本文
定义。

- 官方来源：[Create voice consent](https://developers.openai.com/api/reference/resources/audio/subresources/voice_consents/methods/create)、[List voice consents](https://developers.openai.com/api/reference/resources/audio/subresources/voice_consents/methods/list)、[Create voice](https://developers.openai.com/api/reference/resources/audio/subresources/voices/methods/create)；
- 官方资料复核日期：2026-08-10；账户资格、地区、model、sample 约束与资源 methods 使用前仍须重核。

## 1. Endpoint map

| Method   | Path                                      | 当前 operation |
|----------|-------------------------------------------|----------------|
| `POST`   | `/v1/audio/voice_consents`                | 上传 consent recording 并创建 `audio.voice_consent` |
| `GET`    | `/v1/audio/voice_consents`                | 列出 consent resources |
| `GET`    | `/v1/audio/voice_consents/{consent_id}`   | 读取一个 consent resource |
| `POST`   | `/v1/audio/voice_consents/{consent_id}`   | 更新 consent metadata |
| `DELETE` | `/v1/audio/voice_consents/{consent_id}`   | 删除 consent resource |
| `POST`   | `/v1/audio/voices`                        | 以 audio sample 与 consent 创建 `audio.voice` |

consent create 与 voice create 都是 multipart upload，不是 JSON-only operation。voice create 将声音 sample 与已有 consent identity 关联，
返回的 voice identity 再由 `/v1/audio/speech` 的 `voice` 对象引用。

## 2. Resource 与 operation 边界

consent resource 记录的是授权前置资源，voice resource 记录的是可用于合成的声音 identity；两者不能互换，也不能从 filename 或
display name 推导 opaque id。更新 consent metadata 不等价于替换授权录音或更新 voice sample。

截至快照日期，当前 API Reference 在 voices 下明确展示的是 create operation。不能因 voice consent 具有 list/retrieve/update/delete
就推断 voice resource 也存在同构 CRUD；新增开发必须重新核对当期 reference。

## 3. 安全与证据边界

- consent recording、voice sample、资源 id 与关联 metadata 可能涉及生物特征、身份、授权与隐私，不能写入普通日志或 fixture；
- 文档存在 endpoint 不证明当前账户、地区、组织、model 或 Provider 已获准使用；
- synthetic/mock resource 只能验证 wire shape，不能证明授权有效、声音质量、合规性或真实账户 access；
- 删除 consent 后已有 voice 的行为、retention 与权限影响必须按当期正式 contract 单独验证。
