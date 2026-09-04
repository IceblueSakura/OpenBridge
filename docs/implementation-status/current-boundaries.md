# 当前状态边界

本文是当前 checkout **未实现、未验证和证据适用范围**的全局实施状态 owner。Provider 特定边界由
[providers/](providers/README.md)各分页拥有。它不构成路线图或实施授权；明确非目标仍由
[产品范围需求](../functional-requirements/product-scope.md)拥有，下一项获准行为只由
[当前开发焦点](../implementation-plans/current-focus.md)管理。

## 1. 如何解释这些边界

证据按以下层级分别记录，低层不能替代高层：

1. 静态源码、schema 或编译检查；
2. 确定性 Rust test 与 fixture；
3. Python corpus/testkit 或独立 loopback；
4. 外部 SDK 或独立 curl/Python 客户端；
5. 目标 Agent runtime；
6. 真实 Provider；
7. 负载、长期运行或生产环境。

"未证明"只表示当前证据没有覆盖，不等于已知不可行；"未实现"表示当前 checkout 没有对应 executable contract。带日期的外部结果只适用于记录中的版本、账号、区域、网络、模型和 payload，不提升为长期能力保证。

## 2. 全局未实现范围

当前 checkout 没有以下通用能力：

- 动态 Provider/plugin/Route DSL、request-selected endpoint/credential 和在线控制面；
- 动态 availability/weight、持久化或分布式 cooldown、多进程健康协调；
- 通用异构 conversion policy、完整 OpenAI endpoint/resource catalog 与 response 状态服务；
- 多 Embeddings candidate、Embeddings Bridge、向量转换/缓存/索引/检索和 string tokenizer；
- OTLP logs、内置 Prometheus、指标持久化/查询/重置和分布式 metrics 聚合；
- Responses WebSocket、Realtime、完整 Agent loop、后台 job 和 continuation ledger；
- 多租户控制面、在线用户管理、配额、计费、审计或 GUI。

这些条目同时受产品非目标约束时，不能从本页直接推导实施计划。

## 3. Operation 与协议边界

### Generation 与 Bridge

- Bridge 不支持图片、音频、文件、hosted/custom tool、background/state、opaque continuation 或 Provider 私有语义的通用跨协议转换。
- ToolPlan 的 immutable Inject/Strip 与 Provider-native lowering API 已存在，但 production planner 尚未调用；bounded Gateway web-search loop 仅在 `#[cfg(test)]` 下编译。当前没有 production Gateway tool loop 或普通 function-tool executor。
- 已提交 partial SSE 发生 EOF、body error、timeout 或取消时，网关只能终止当前 body 并记录失败，不能安全改写 HTTP status、注入第二条 stream 或伪造 terminal。
- 当前确定性 transport/loopback 不证明真实网络下 retry/fallback 的吞吐、公平性、SLA、多进程恢复或长期稳定性。
- 外部 OpenAI SDK、Codex/Hermes runtime、长 reasoning stream、真实反向代理和强制后备 source 未形成统一当前验收。

### Models 与 capability preflight

- 没有动态 capability negotiation、request-selected Route 或运行时 capability routing。
- `prompt_cache_key` 是 accepted best-effort hint，可能按 candidate 删除；cache hit、成本、延迟、active retention、options 和 breakpoint 未实现或未证明。
- serial-only Provider 的 `parallel_tool_calls:false` 安全省略合同尚未注册；当前 active true/false 都只在固定 interface 已证明可控制并行调用时接受。
- Models/Target catalog 不能证明 credential 可用、网络可达、配额、账号 entitlement 或模型质量。

### Embeddings

- 当前只有单 Route Native execution；没有跨 Route fallback、Bridge、数值向量转换、缓存、索引或检索。`bailian/qwen3-7-text-embedding` 的 target/API-scoped float32/Base64
  wire re-encoding 只改变表示，不执行归一化、降维或模型转换。
- `qwen3.7-text-embedding` 的北京 OpenAI-compatible float、1024/512 维、20 条 batch 和基础中英排序已有带日期小样本；Hindsight
  SDK Base64 路径在升级前线上版本仍被 preflight 拒绝，修复后的部署态、完整 Hindsight runtime、语义 benchmark、生产配额、负载和长期网络可用性仍未验证。

### Native 图片与文件输入

- 图片 Bridge、Pro 图片、`file_id`/Files、image edit、Provider-side DNS/redirect/MIME/size、OCR、内容安全和显式 detail 未证明；
  各 Provider 的具体媒体边界见[对应 provider 页](providers/README.md)。
- OpenBridge 不下载、解析、转换、转码、缓存或扫描远程图片/文件。
- 当前生产 Public Model 不公开 file input；synthetic file loopback 不证明真实模型/backend、Provider 下载行为、解析质量、费用或 SDK/Agent 兼容。

### Native 音频

- OpenAI `/v1/audio/*`、Responses audio、Realtime、remote/multiple audio、更多格式/语言/voice、媒体质量、voice authorization/store 和跨请求 voice identity 未证明。
- 五种 MiMo 音频 task 的真实下游网关复测、播放器/硬件验收、负载和长期运行未完成（见 [mimo](providers/mimo.md)）。

### Images Generations

- I2I/edit/variation、异步任务轮询、stream 输出和 `b64_json` 未实现。
- Images 当前不复用 Generation/Embeddings recovery runner；单请求可能已计费，因此没有 retry、fallback 或 credential rotation。
- 图像 URL 是 Provider 返回的临时签名 URL；OpenBridge 不下载、缓存或延长有效期。
- 真实 OpenAI Images SDK、图像内容质量、计费语义、配额、内存峰值、SLA、负载、长期取消和生产 logging 未验证。

### MCP

- 当前本地 tool 只有 `hello`；没有 hosted tool、MCP Tool Bridge 或由 generation gateway 执行普通 function tool。
- 进程内 MCP contract 不证明外部 MCP SDK、浏览器、反向代理、工具安全、真实网络部署、负载或长期运行。

## 4. Probe 边界

- 每次 Generation probe 只执行一个显式 unit case；当前 CLI/库不拥有跨 protocol、delivery、reasoning 或 capability 的矩阵编排。
- `reasoning-summary`、`include-encrypted-content`、`prompt-cache-key` 是 Responses-only 单字段差分：只发送被探测字段，接受性由
  outcome 体现；它们不验证 summary 质量、加密内容语义或缓存效果，与 Chat 协议组合在选择阶段拒绝。
- 管理员可以为非 tool case 覆盖固定用户 prompt，为 JSON Schema case 覆盖响应格式对象与名称；覆盖只改变请求文本，不改变
  闭合 case 集合、tool 定义、图片负载、output ceiling 或 oracle 结构。带自定义 `--schema` 的 case 因无固定 oracle 恒为
  `inconclusive`，报告只携带覆盖内容指纹与 `--schema-name`，evidence 归属由外部脚本记录指纹与原文的对应。
- 当前 function-tool probe 只验证固定 prompt 下单次首轮的 tool choice、strict arguments 与 parallel call 差分；它不执行工具、不发送
  tool result、不做续轮或 Agent loop，也不证明工具调用长期稳定。当前图片 probe 仅覆盖固定 inline PNG OCR case；一次识别成功不证明
  remote URL、detail、其他格式、多图、视觉质量或长期稳定性，文件、音频和视频 probe 尚未进入本阶段。
- 一次 `accepted` 或 capability oracle 的 `supported` 只证明该固定首轮请求当时取得相应 JSON/SSE 结果；不证明 reasoning 参数实际生效、
  完整工具调用流程、工具执行/续轮、能力稳定、模型质量、SDK/Agent 兼容、负载或长期稳定性。

## 5. 观测、配置与生产边界

- 确定性配置测试不证明 credential 有效、Provider 可达、OAuth authority/refresh 长期稳定、collector/sink 可用或生产日志保留策略正确。
- 当前没有 OTLP logs、内置 Prometheus、dashboard/告警、metrics 历史数据库或多进程聚合。
- 本地 JSONL content snapshot 是受控开发能力；没有生产敏感流量、资源开销、磁盘故障、负载或长期运行验收。
- 当前没有真实 Provider wire dump；普通 telemetry 不用于计费准确性、Provider SLA 或业务正文审计。

## 6. 测试资产边界

当前确定性测试和 corpus 能证明 registry、routing、wire、Generation Static/Event IR lifecycle、SSE fragmentation、retry/fallback/cooldown、取消，以及全部 51 个 canonical wire case 经过 production Router 的目录驱动回放（`tests/catalog_replay_contract.rs`），但不证明：

- 完整 Model/Provider inventory、retired ID 黑名单、完整 candidate 数量/顺序或每个 catalog capability fact；
- 每个 Provider/model 组合都重复经过 Native/Bridge production Router，或 OTLP metrics exporter 拥有独立进程级集成覆盖；
- 3 个 stream-violation case（`event_type_conflict`、`terminal_violation`、`incomplete_arguments`）的 proposed oracle——回放当前锁定生产终止行为，合成终态注入仍待产品裁决；
- canonical oracle 等于完整 OpenAI API；
- hosted/custom tool、continuation、媒体和 Provider 私有扩展可转换；
- 真实 SDK、Agent、Provider、TLS/HTTP2、并发背压、负载或真实 packet boundary 兼容；
- semantic reference trace、synthetic context byte/position sweep 或 strict JSON oracle 不证明真实 model 的 context limit、tokenizer、推理质量、Provider 原生 enforcement 或 OpenBridge production path 已执行；
- 外部来源未来保持相同行为。

尚未实施的测试切片（目录驱动回放之外的已知缺口）：

- 缓存管理没有独立 corpus case：`prompt_cache_key` 仅有候选投影/省略测试（`forwarding_contract/resilience.rs`）与 usage 解析测试（`observability_contract.rs`），没有 wire-level cache hint case，也没有登记对应 `sources/` 条目；
- 14 个 `semantic-cases/` 没有 OpenBridge production runner：回放契约不消费它们，4 方向 × semantic case 的 normalized trace 判定尚未实施；
- 3 个 stream-violation case 的 proposed oracle（合成终态注入）与生产终止行为的裁决仍开放，见上一条边界。

corpus 中未固定 source ref、pending license 与 `reviewed` case 必须继续显式暴露，不能改写为完成状态。实际执行过的外部证据以[evidence 索引](evidence/README.md)为准。
