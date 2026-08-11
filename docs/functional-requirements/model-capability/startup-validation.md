# 启动时校验

## 状态

本文是[模型与能力契约域](README.md)的启动校验模块：定义 registry 在监听前必须拒绝的非法事实组合。
其他模块见[模型与能力契约域](README.md)导航。

## 1. 启动时拒绝项

registry 必须在监听前拒绝：

- 缺少 canonical task，或 task variant 与其 payload、固定 modalities/reasoning 语义矛盾；
- 非法 Public Model id、零值 `created`、空白展示字段或不一致生命周期时间；
- total/input/output context 为零，或输入/输出上限超过 total context；
- 显式模态集合为空或重复；
- 空 Route 列表、重复 Route 或未知引用；
- Upstream API 规则扩大 canonical Model、收窄后产生不一致事实，或普通忽略参数未由 Model 声明、重复、与禁用字段重叠、绑定 Embeddings；Embedding identity、dimension、encoding 或 input-form 声明矛盾；
- Chat/Responses 媒体 source/format/detail/media type 集合为空、重复、协议错配，或 limits 为零/相互矛盾；
- Structured Output profile 为空、重复、把 strict 与不含 JSON Schema 的 mode 组合，或 executable profile 超过 Provider ceiling；
- reasoning checked set 或 wire mapping 不一致，以及 ordinary parameter 重新声明协议 reasoning alias；
- 非 generation Public Model 配置正向 reasoning level 归一化策略；
- Provider audio ceiling 为空、重复 task、缺少该 task 的完整 input/output/conditioning/delivery payload，或把 Provider ceiling 当成
  单个 executable profile；generated-audio profile 缺少必填 JSON 或 SSE delivery；
- Upstream API 能力超过 Provider contract 上界；通过 ceiling 后，operation/executable profile 与 canonical task 不兼容；
- Responses `include` capability set 含重复值或超过 Provider ceiling；
- 一个 Public Model 混合不同 canonical task，或同 task/same audio variant 的必需 payload 交集为空。

## 关联文档

- [模型与能力契约域导航](README.md)
- [事实所有权与公开边界](fact-ownership-and-boundary.md)
- [模型事实与固定接口契约](model-facts-and-interface-contract.md)
- [配置与凭证](../configuration-credentials/README.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
