# 下一步目标

**用仅文本 Generation 验证 IR 化网关整体流程，再扩展 Provider 和多模态。** 产品方向见 [v2 目标](../architecture-v2/README.md)，当前完成度只由[实施基线](../architecture-v2/migration.md)维护。

最小 loopback HTTP 入口、可信 Public Model/task、预算变换、固定候选、Provider HTTP、增量 body 与固定 SDK synthetic 续轮已有接线。启动合同见 [HTTP 指南](../http-gateway.md)；不再把这些基础列为待从零建立，也不把 synthetic 成功写成生产兼容。

## 优先顺序

| 优先级 | 建议切片 | 退出条件 |
|---|---|---|
| 1 | 已接入 Provider 的剩余受控外部验收与账号阻塞 | 选定现有正式绑定与客户端，明确账号、精确请求矩阵/预算/输出边界；请求实际经过 binary/Router，而非绕入口的库级 probe，覆盖工具续轮与可观察失败 |
| 2 | 选定 Agent/缓存场景与必要文本投影 | 对具体客户端核对稳定前缀、Schema/工具顺序、replay scope 与派生 view；按场景补 Chat content_filter、service-tier 请求/metadata 等。若评价缓存效果，独立设计对照，不把 usage 默认零当计费事实 |
| 3 | 与实际使用相称的运行保障 | 按已观察需求决定凭据生命周期、诊断、负载与失败策略；未选定前不预建动态 registry、通用插件或完整旧运行时 |
| 4 | 扩展 Provider 与多模态 | 新 wire 差异改 adapter，真正的新能力演进共享 task/extension owner；同时验收 request/response/event 与资源边界 |

[OpenRouter Luna 的 binary/SDK 证据](../implementation-status/evidence/2026-09-29-openrouter-luna-acceptance.md)已覆盖选定文本/工具正常路径；[reasoning 专项](../implementation-status/evidence/2026-09-29-reasoning-continuation-acceptance.md)还覆盖了该目标的两轮加密状态回放。后续优先选择尚未覆盖的真实失败路径、带实际加密状态的工具/更长续轮或具体 Agent/缓存场景，不把相同成功矩阵无限重复。

[API-key 接入](../architecture-v2/api-key-text-profiles.md)已建立固定绑定和 pi 所需的 Chat 文本数组入口。[Flash / LongCat / 百炼验收](../implementation-status/evidence/2026-09-30-flash-longcat-bailian-acceptance.md)之后，不再把新增 Flash、LongCat 原生 Responses 接线或百炼换 key 作为待办。下一批如获授权，应优先定位 LongCat 响应头前的工具请求超时并补齐其 SSE，再核对 Flash/百炼极低输出预算的失败边界，而非重复已通过的正常矩阵；不以延长 deadline、自动 retry 或 codec 宽松化掩盖问题。Kimi 仍暂停调用但保留产品绑定，NVIDIA 的已观察失败按下段跟进。MiMo 可显式对比 minimal，但不静默改变已请求的 effort。不把尚未准入的其他 native Responses、Codex/OAuth 或全量模型目录混入已完成绑定。

[NVIDIA 定向边界](../implementation-status/evidence/2026-09-30-nvidia-boundaries.md)已区分默认 JSON 预算截断、显式 none 的可用对照，以及仍未定位的续轮 502。[Probe 收敛](../probes.md)已提供共享预算、仓库内 pi 与原调用诊断，不再把这些基础设施写成待重建。后续需要新授权批次时，针对已定位到 intake 的 NVIDIA HTTP-200 SSE 失败获取最小独立反例，再决定是否修改 adapter；历史错误不自动认定同因。不重复已通过矩阵来替代根因定位，不把未观察到 429 当作限流验收。

后续更多仅支持 `reasoning_content` 的模型按可读 reasoning profile 验收：分别检查正文/推理归属、工具历史、JSON/SSE 与变换保真，不要求其生成密文，也不借用加密状态的来源约束。遇到独立签名或 opaque continuation 时再固定其格式和 owner 合同；不为尚未选定的模型预建通用透传。

历史[库级 Flash 证据](../implementation-status/evidence/2026-09-29-flash-provider-adapter-acceptance.md)仍只证明当时库级路径；现有正式绑定与 binary 验收分别由 catalog 和上述新证据维护。真实调用与付费请求仍需独立授权；方向文档不授予账号、部署、提交或推送权限。

## 下一片需选定的边界

- 使用哪个已绑定模型、哪个真实客户端，以及同协议还是明确可表示的跨协议子集。
- 输出 token/请求次数、脱敏报告边界；不复用 synthetic keys 或把旧私有配置当新格式。
- 检查 SDK → 入口认证 → IR → Provider → 实际下游 body → 下一轮回放的整条路径，而非仅验证 HTTP 200。
- 保留请求不被透传为任意 URL/credential/header、late failure 不伪造终态、交付后不 retry/fallback 的边界。
- 新发现先区分 Provider 输出异常、字段投影缺口、I/O 生命周期问题和客户端差异，再在 [current-focus](current-focus.md)选择可验收切片。

不以 hosted tools、program 执行、Codex turn 管理、state/WS、媒体或其他 task family 完整实现为前置条件，也不因本机入口已存在而宣称上线验收完成。
