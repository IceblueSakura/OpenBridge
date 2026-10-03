# OpenAI API 规范导航

本页定位官方资料与本地 operation 来源，不复制 endpoint/字段库存、兼容矩阵或动态能力清单。细粒度 owner 见[OpenAI 索引](README.md)；采用边界见[Operation 与验证](endpoint-adoption-and-fake-testing.md)。

## 既有来源基线

既有页面核对日期为 **2026-08-10**；机器可读资料的首次采集日期为 **2026-07-18**，基线为 OpenAPI `3.1.0`、当时 `info.version=2.3.0`。来源是 [openai/openai-openapi](https://github.com/openai/openai-openapi)与[API Reference](https://developers.openai.com/api/reference)。这些标记不是当前在线版本或本地实现声明，本地整理不刷新日期。

| 资料 | 责任 |
|---|---|
| OpenAPI | method/path、基础参数/response schema与生成客户端形状 |
| API Reference | 字段语义、resource method 与 error |
| Guides | transport、tool loop、state、background 与 media 生命周期 |
| Model docs / changelog | 需现场核对的模型、限制、枚举与 beta/deprecated 状态 |

SDK 是消费者与类型交叉核对来源，不是独立 wire authority。Schema shape 不能替代能力、权限、资源 retention 或闭合语义。

## 无本地细粒度 owner 的官方入口

这些仅为按需查询导航，不暗示采用或支持；与已有 Chat/Responses/media/resource 文档重复的路径只从[索引](README.md)进入。

- 其他推理/发现：[Models](https://developers.openai.com/api/reference/resources/models/methods/list)、[Legacy Completions](https://developers.openai.com/api/reference/resources/completions/methods/create)。
- 独立 beta：[Responses Multi-agent](https://developers.openai.com/api/reference/resources/beta/subresources/responses/methods/create)；不得按名称套用稳定 Responses 合同。
- 异步作业：[Batches](https://developers.openai.com/api/reference/resources/batches/methods/create)、[Evals](https://developers.openai.com/api/reference/resources/evals/methods/create)、[Fine-tuning](https://developers.openai.com/api/reference/resources/fine_tuning/subresources/jobs/methods/create)、[Webhooks](https://developers.openai.com/api/reference/resources/webhooks)。
- 托管产品：[ChatKit](https://developers.openai.com/api/reference/resources/beta/subresources/chatkit/subresources/sessions/methods/create)、[Containers](https://developers.openai.com/api/reference/resources/containers/methods/create)、[Skills](https://developers.openai.com/api/reference/resources/skills/methods/create)、[Content provenance](https://developers.openai.com/api/reference/resources/content_provenance_checks/methods/create)。
- 独立管理与 legacy 来源从[官方总目录](https://developers.openai.com/api/reference/overview)按需定位；不能从目录存在推定一般模型转发合同。

动态入口使用前重新固定具体 operation、版本、认证和 lifecycle；本页不替代该调查，也不构成真实请求授权。已有关闭前的 Videos 资料用途与时效仅由[视频索引](README.md#8-视频)维护。
