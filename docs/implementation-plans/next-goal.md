# 下一步目标

**以文本 Generation 主链为基础，按选定的图片输入→文本输出 slice 继续扩展 IR 化网关；后续媒体按资源来源、content owner 与任务分别准入。** 产品方向见 [v2 目标](../architecture-v2/README.md)，当前 `main` 基于原 `semantic-v2` 独立演进，不以迁移、追平或恢复旧版为目标；语义实现与产品目标的差距由[当前能力与边界](../implementation-status/generation.md)维护。

最小 HTTP 入口的职责和运行边界见 [HTTP 指南](../http-gateway.md)。当前 Provider/模型与协议准入按 [AGENTS.md](../../AGENTS.md#current-provider-model-and-compatibility-information)现场查询；本页不维护支持清单、实测结果或临时账号阻塞。

## 优先顺序

| 优先级 | 建议切片 | 退出条件 |
|---|---|---|
| 1 | 按实际需求选择端到端文本场景 | 现场核对绑定、客户端和授权范围；请求经过 binary/Router，覆盖 IR、交付、工具续轮与失败边界，不仅是库级 decode 或 HTTP 200 |
| 2 | 选定 Agent/Provider 原生缓存场景与必要文本投影 | 核对稳定前缀、Schema/工具顺序、replay scope 与派生 view；按消费需求补投影。评价缓存效果时独立设计对照，不把兼容默认值当计费事实 |
| 3 | 与实际使用相称的运行保障 | 按具体需求决定凭据生命周期、诊断、负载与失败策略；未选定前不预建动态 registry、通用插件或完整旧运行时 |
| 4 | 扩展 Provider 与多模态 | 以现有 user URL/inline 图片输入为起点，按需求选定 file_id 来源/生命周期、工具媒体结果、其他模态或独立媒体任务；新 wire 差异改 adapter，真正的新能力演进共享 task/extension owner，同时验收 request/response/event 与资源边界 |

不以重复成功矩阵代替问题定位，也不根据过期结果固定下一轮目标。新发现先区分上游输出、字段投影、I/O 生命周期和客户端差异；稳定结论进入 owning code 注释与独立 synthetic 回归，运行结果只在当次交付和授权 run 中保留。

可读 reasoning 与 opaque continuation 分别按实际合同验收：前者核对正文/推理归属、历史和变换保真，不要求密文；后者需要明确格式、owner、origin 与 finality。不为尚未选定的模型预建通用透传，不静默修改请求控制来通过测试。

缓存亲和范围限定为 Provider 自动缓存与明确的 cache key/session carrier 投影，不在本项目实现负载均衡、回答缓存、会话管理或跨请求粘性路由。稳定公开接入与扩展 owner 见 [ADR 0011](../architecture-v2/decisions/0011-stable-admission-provider-cache.md)。

## 下一片需选定的边界

- 从当前源码和启动入口确认模型、客户端及同协议或可表示的跨协议子集；实际实例启用与账号可用性分开判断。
- 若需真实调用，先约定目标、精确请求矩阵、token/请求数与脱敏边界；方向文档、旧计划和存在密钥都不是授权。
- 检查客户端 → 入口认证 → IR → Provider → 实际下游 body → 下一轮回放的整条路径。
- 保持固定可信目标、预算、late failure 不伪造终态、交付后不 retry/fallback 的边界；不靠延长 deadline 或宽松解析掩盖失败。
- 在 [current-focus](current-focus.md)记录获准行为切片，以独立反例驱动实现；完成后恢复为空，不留下测试日记。

无需把 hosted tools、program 执行、Codex turn 管理、state/WS 或其他 task family 的完整实现当作每个切片的前置条件；也不因本机入口存在而宣称生产就绪。
