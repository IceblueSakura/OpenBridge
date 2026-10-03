# 下一步目标

下一步设计重点是 **Agent-first、协议中立的 Generation 交互与依赖合同**，不是追齐某家 API 字段。项目处于未发布的设计探索阶段，可在获准切片内替换类型；一套 IR 指共享原则与 task family，而非万能请求。设计权威归 [semantic IR](../architecture-v2/semantic-ir.md)，未闭合实现边界归 [Generation 缺口](../implementation-status/generation.md)。

本页只维护未完成方向，不授予实施或真实调用权限。具体行为、失败用例和验收范围写入 [current-focus](current-focus.md)。Provider/model、激活及上游准入按 [AGENTS 查询方法](../../AGENTS.md#current-provider-model-and-compatibility-information)现场核对，不在计划中保存清单或测试结果。

## IR 设计优先顺序

| 优先级 | 设计主题 | 定稿边界 |
|---|---|---|
| P0 | 交互、结果与控制转移 | item/message/response/turn/group/call/result；区分响应闭合、产物完整、turn 进度与 continuation，不把表示控制转移当作调度授权 |
| P0 | Identity、分组与 replay 依赖 | 稳定 identity、wire 坐标、单 owner 与组/前缀依赖；opaque scope/finality、编辑失效及客户端交付→回传合同 |
| P1 | 内容、工具结果与资源 | 媒体值和用途、结构化结果与工具错误、资源来源/权限边界、引用的 owner 与来源坐标；不建任意可执行容器 |
| P1 | 控制、Schema、cache 与 usage | 区分模式/预算/显示、声明约束/目标保证、亲和提示/前缀策略/资源引用、reported counts 与有前提的派生 view |
| P2 | 上下文演进 | 配置变化、compaction、远端 continuation 资源的 scope 与依赖；不引入 session 服务或自动 Agent loop |

### 定稿方法与下一片选择

1. 先定义概念、唯一 owner、presence、合法状态和关系，再定 Rust 类型。共享概念不复制到 Provider IR，也不以现有 wire 为语义上限。
2. 按[固定来源](../references/README.md)与独立官方合同检查 request/history、response/event 和跨协议可表示性；资料或类型存在不代表准入。
3. 明确插入、替换、删除、重排造成的依赖失效；为所选切片确定 wire/API 版本、下游 carrier、scope、资源限制及是否允许损失转换。未知语义不能塞入万能 metadata。
4. 优先选择能检验工具续轮、response/turn 分离和 replay 依赖的最小端到端场景，再扩展媒体；不要求预先实现所有 Provider、任务或工作流。

可读 reasoning 与 opaque continuation 分别验收：可读内容不能替代必要 signature，opaque 值必须定义格式、owner、origin、依赖和 finality。无 opaque 合同不强造密文；有回放要求则需验证真实交付→回传。不能静默修改控制或丢弃不可表示内容以通过测试。

## 实施方向

| 优先级 | 建议切片 | 退出条件 |
|---|---|---|
| 1 | 授权与订阅执行扩展 | 在[凭据组件当前合同](../architecture-v2/decisions/0012-grok-personal-credential-pool.md)上按实际需求选定 client/部署用途与订阅准入；新增 driver 或自动生命周期策略需独立合同，不混用 grant/profile、workspace 或 replay scope |
| 2 | 端到端文本场景 | 经实际 binary/Router 检验 IR、交付、工具续轮与失败边界；不以库级 decode 或 HTTP 200 代替验收 |
| 3 | Agent/Provider 原生缓存与必要文本投影 | 检验稳定前缀、Schema/工具顺序、scope 和派生 view；缓存效果使用独立对照，不把默认值当计费事实 |
| 4 | 运行保障 | 按使用需求选择凭据生命周期、诊断、负载与失败策略；不预建动态 registry、通用插件或调度框架 |
| 5 | Provider 与多模态 | 在已选图片输入场景上定义 file_id、资源生命周期、更广工具媒体和独立 task family；wire 差异归 adapter，真正的新能力归共享语义 owner |

认证生命周期与推理扩展不以全部 IR 设计完成为前置。现行 access 借用和操作方式归[凭据指南](../credentials.md)；新的 scope/resource、callback/client、refresh 或账户调度策略须独立定稿。身份验证不证明推理资格，401 不授权跨账户切换，普通请求不发起交互登录，也不搜索第三方认证缓存。

缓存只使用 Provider 原生功能和明确 carrier；不实现回答缓存、负载均衡、会话管理或跨请求粘性路由。不要把 hosted tools、program 执行、Codex turn 管理或 WebSocket 的完整实现当作每个切片的前置条件。

## 下一片需选定的边界

- 明确 public model、客户端、原生协议或可表示的跨协议子集；注册、激活与账号可用性分开验证。
- 验收客户端 → 入口认证 → IR → Provider → 实际下游 body → 下一轮回放，保留固定目标、绝对预算、提交后禁止重放和真实终态约束。
- 先用独立 synthetic 反例验证；真实请求另行约定目标、矩阵、请求/token 预算与脱敏范围，存在凭据或计划不构成授权。
- 新问题按上游输出、投影、I/O 生命周期和消费者差异定位，不重复成功矩阵或靠宽松解析/延长 deadline 掩盖失败。稳定约束归 owning code，运行结果只在当次交付和获准 run 中保留。
