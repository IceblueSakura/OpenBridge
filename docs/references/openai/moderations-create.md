# OpenAI Moderations Create 调研

## 来源、范围与快照

本文只记录 `POST /v1/moderations` 的 JSON request union 与 JSON result。它不定义网关级自动拦截策略，也不把模型分数解释为跨
Provider 通用的安全阈值。

- 官方来源：[Create moderation](https://developers.openai.com/api/reference/resources/moderations/methods/create)、
  [Moderation guide](https://developers.openai.com/api/docs/guides/moderation)；
- 官方资料复核日期：2026-08-10；动态 model、category、input-type coverage 与政策语义使用前仍须重核。

## 1. Request

```text
POST /v1/moderations
Content-Type: application/json
```

request 的核心字段为 `input` 和可选 `model`。`input` 是闭合 union：

- 单个 string；
- string array；
- 由 `{ "type": "text", "text": ... }` 与
  `{ "type": "image_url", "image_url": { "url": ... } }` 组成的多模态 object array。

image URL 可以引用普通 URL 或 Base64 data URL。输入是数组时，result 顺序和数量属于 contract；不能把多条输入合并成一次本地文本
扫描后只返回一个结果。

具体 model 是否接受 text/image、某 category 是否适用于某种 input type，均由该 model 的当期能力决定。请求 schema 允许自定义
model string，不代表任意 generation model 都提供 Moderations operation。

## 2. Result

成功结果包含 request `id`、实际 `model` 和 `results[]`。每个 result 至少组合：

- `flagged` 总体布尔值；
- `categories` 中逐 category 的布尔判断；
- `category_scores` 中逐 category 的数值；
- `category_applied_input_types` 中该 category 实际覆盖的 `text`/`image` 来源。

category 集合及其 nullable/适用输入类型可以随 model 和兼容新增演进。consumer 不应把某个历史 category 列表当作永远封闭的全局枚举，
聚合层也不能丢弃上游已经返回但本地尚未知的新 category。

## 3. 聚合与 fake 边界

Moderations 是单次 JSON request/response，适合先用确定性 fake 固定：

- string、string array 和多模态 object array 的解析与拒绝；
- batch input 与 `results[]` 的数量、顺序和逐项字段；
- text-only 与 text/image capability preflight；
- upstream HTTP/error envelope、超时、取消和安全重试分类；
- 未知新增 category/property 的前向兼容。

fake category、score 或 `flagged` 只能证明 wire contract。它不能证明真实分类准确率、政策适用性、阈值选择、跨 model 可比性或
Provider 的安全承诺。真实接入必须固定到具体 Provider、operation、model 与输入模态，并另行保存脱敏的真实证据。

## 4. 与网关策略的区别

向客户端暴露 `/v1/moderations` 与在网关内部自动审核所有请求是两项不同的产品行为。后者会改变隐私、延迟、计费、误拦截、失败策略和
数据传输范围，不能仅因实现了该 endpoint 而隐式开启。
