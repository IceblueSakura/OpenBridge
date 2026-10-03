# Operation 与验证边界

本页保留选择协议来源与验证层的边界，不维护 API family 库存、网关适配性排名或实施计划。具体 operation 的来源导航归[OpenAI 索引](README.md)，额外官方入口归[规范目录](api-specification-catalog.md)；本地设计和准入分别归[语义架构](../../architecture-v2/README.md)与[实施缺口](../../implementation-status/generation.md)。

## 来源边界

既有官方页面基线为 **2026-08-10**，入口为 [API Overview](https://developers.openai.com/api/reference/overview)。OpenAI 的 `v1` 可新增 resource、optional parameter、response property 和 stream event；路径、字段、beta/deprecated 状态不能从旧资料推定当前可用。实施前固定所选 API/schema/SDK/profile 与许可，本地整理不刷新外部核验日期。

官方目录同时包含推理、托管资源、异步作业、双向会话与管理面。出现在一个目录中不使它们成为一个共享 task，也不自动进入 OpenBridge 产品承诺。Legacy 或 beta 采用需要明确 consumer 和独立版本合同，不从同名 operation 推定与稳定 API 等价。

## Operation 不只是 method/path

采用前至少区分：

- method/path/query/header、认证域和 Content-Type；
- request encoding：JSON、multipart、bytes、SDP 或双向 event；
- response transport 与闭合：JSON、binary、SSE、WebSocket、WebRTC 或 SIP；
- error、首字节后的失败、取消及 replay 边界；
- model/task、资源 issuer/account、opaque identity 与 lifecycle；
- allocation/body/media 预算、权限和敏感数据。

同一模态不证明 operation 等价；图片输入、独立图片生成和 hosted image tool 分别有 owner。资源引用不能透明迁移到另一账户或 Provider；管理面 credential 不属于普通推理认证域。

## 验证选择

独立 oracle、语义/字节/消费者/外部执行分层和资产许可规则只由[验收基线](../conformance-baseline.md)维护。测试按 operation 的实际风险覆盖媒体 framing、资源 lifecycle、异步 job 或双向 session，不能以固定 `200` JSON body 代替。

Synthetic 只证明被执行的 wire、budget 与状态边界，不证明模型质量、账户资格、真实 retention、配额、费用、网络或长稳。Provider 自称 OpenAI-compatible 也不能替代逐 operation 验证；结果在当次交付报告，不写入来源导航或长期记忆。
