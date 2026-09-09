# 当前状态边界

本文是当前 checkout **当前实现限制、未验证范围和证据适用范围**的全局实施状态 owner。Provider 特有边界由
[Provider 分页](providers/README.md)拥有；永久非目标由[产品范围](../functional-requirements/product-scope.md)拥有。本页不构成路线图或实施授权，[当前开发焦点](../implementation-plans/current-focus.md)只记录用户已批准的行为范围，不独立授权。

## 1. 如何解释这些边界

证据分层见[开发指南](../development.md#证据与外部验证)，低层不能替代高层。

“未证明”只表示当前证据没有覆盖，不等于已知不可行；“未实现”表示当前 checkout 没有对应 executable contract。带日期的外部结果只适用于记录中的版本、账号、区域、网络、模型和 payload，不提升为长期能力保证。

## 2. Operation 实现与验收边界

以下区分已实现路径的限制、未接入机制和未验证场景。列出某个缺口不表示它在产品范围内，也不构成补齐承诺。

### Generation 与 Bridge

- Native JSON 与 SSE 都先进入核心 IR 再编码，已覆盖空输出、refusal 和非完成结果；中断工具参数不会伪装为完整 JSON 参数。Native 合法源字段、annotation 和未知非终态事件以有界 codec envelope/扩展保留，跨协议不猜测其含义。
- SSE 规范化覆盖分片、CRLF、data-only typed event、可确定的 event/type 补齐，以及由已验证 items 补齐稀疏 completed terminal。它不承诺恢复任意缺失身份、乱序或丢失的消息边界；矛盾 type/event、非法 JSON、超限和无 terminal 仍拒绝。未执行真实 Provider 或负载兼容性复测。

- Bridge 不支持图片、音频、文件、hosted/custom tool、background/state、opaque continuation 或 Provider 私有语义的通用跨协议转换。
- ToolPlan 的 immutable Inject/Strip 与 Provider-native lowering API 已存在，但 production planner 尚未调用；bounded Gateway web-search loop 仅在 `#[cfg(test)]` 下编译。当前没有 production Gateway tool loop 或普通 function-tool executor。
- 已提交 partial SSE 发生 EOF、body error、timeout 或取消时，网关只能终止当前 body 并记录失败，不能安全改写 HTTP status、注入第二条 stream 或伪造 terminal。
- `prompt_cache_key` 只形成 accepted best-effort hint，可能按 candidate 删除；cache hit、成本、延迟、active retention、options 和 breakpoint 未实现或未证明。
- serial-only Provider 的 `parallel_tool_calls:false` 安全省略合同尚未注册；当前 active true/false 都只在固定 interface 已证明可控制并行调用时接受。
- 当前确定性 transport/loopback 不证明真实网络下 retry/fallback 的吞吐、公平性、SLA、多进程恢复或长期稳定性；外部 OpenAI SDK、Codex/Hermes runtime、长 reasoning stream、真实反向代理和强制后备 source 也未形成统一当前验收。

### Models、Provider 与 capability preflight

- Models/Target catalog 不能证明 credential 可用、网络可达、配额、账号 entitlement 或模型质量。
- Provider 页记录的注册能力与外部 probe 差异仍可能需要独立获准的代码收窄；差异记录不自动授权修改注册。

### Embeddings

- 当前只有单 Route Native execution；没有跨 Route fallback、Bridge、数值向量转换、缓存、索引或检索。`bailian/qwen3-7-text-embedding` 的 target/API-scoped float32/Base64 wire re-encoding 只改变表示，不执行归一化、降维或模型转换。
- Qwen Embeddings 与 Hindsight 的历史 Base64 阻断及修复边界见[Bailian 证据](evidence/2026-08-29-openbridge-qwen37-embeddings-hindsight-compatibility.md)。修复后的部署态、完整 Hindsight runtime、语义 benchmark、生产配额、负载和长期网络可用性仍未验证。

### Native 图片、文件与音频

- 图片 Bridge、Pro 图片、`file_id`/Files、image edit、Provider-side DNS/redirect/MIME/size、OCR、内容安全和显式 detail 未证明；具体媒体边界见对应 Provider 页。
- OpenBridge 不下载、解析、转换、转码、缓存或扫描远程图片/文件；当前生产 Public Model 不公开 file input。synthetic file loopback 不证明真实模型/backend、Provider 下载行为、解析质量、费用或 SDK/Agent 兼容。
- OpenAI `/v1/audio/*`、Responses audio、Realtime、remote/multiple audio、更多格式/语言/voice、媒体质量、voice authorization/store 和跨请求 voice identity 未证明。
- 五种 MiMo 音频 task 的真实下游网关复测、播放器/硬件验收、负载和长期运行未完成（见 [MiMo](providers/mimo.md)）。

### Images Generations

- I2I/edit/variation、异步任务轮询、stream 输出和 `b64_json` 未实现。
- Images 当前不复用 Generation/Embeddings recovery runner；单请求可能已计费，因此没有 retry、fallback 或 credential rotation。
- 图像 URL 是 Provider 返回的临时签名 URL；OpenBridge 不下载、缓存或延长有效期。
- 真实 OpenAI Images SDK、图像内容质量、计费语义、配额、内存峰值、SLA、负载、长期取消和生产 logging 未验证。

### MCP

- 当前本地 tool 只有 `hello`；进程内 MCP contract 不证明外部 MCP SDK、浏览器、反向代理、工具安全、真实网络部署、负载或长期运行。

## 3. 产品范围与非目标

永久非目标由[产品范围的能力层级与准入条件](../functional-requirements/product-scope.md#6-能力层级与准入条件)定义；
[暂不纳入产品承诺](../functional-requirements/product-scope.md#7-暂不纳入产品承诺)另行列明当前不承诺的能力，两者不能混同。内部类型或 test-only 机制存在，不会把产品非目标变为待实施事项。

## 4. Probe 边界

参数与固定 case 的行为说明见[Provider 探测指南](../guides/provider-probing.md)，本页只保留验证缺口：

- 单次首轮 probe 不执行工具、不发送 tool result、不续轮，也不证明完整 Agent loop 或长期工具稳定性。
- 固定 inline PNG 成功不证明 remote URL、detail、多图、其他格式或视觉质量；文件、音频和视频 probe 尚未接入。
- Responses 差分接受不证明 reasoning 实际生效、summary 质量、加密内容语义或缓存效果。
- 自定义 schema 没有固定 oracle，不能把请求被接受提升为 schema enforcement 验收。
- `accepted`/`supported` 只适用于当时的固定请求，不证明 SDK/Agent、负载或长期兼容。

## 5. 观测、配置与生产边界

- 确定性配置测试不证明 credential 有效、Provider 可达、OAuth authority/refresh 长期稳定、collector/sink 可用或生产日志保留策略正确。
- 当前没有 OTLP logs、内置 Prometheus、dashboard/告警、metrics 历史数据库或多进程聚合。
- 本地 JSONL writer 已有 Linux `/dev/full` 真实写入失败、后续 snapshot 丢弃和有界 shutdown 回归；Router smoke 验证 JSON/SSE 业务响应保持不变。它仍没有生产敏感流量、真实磁盘耗尽、资源开销、负载或长期运行验收。
- 当前没有真实 Provider wire dump；普通 telemetry 不用于计费准确性、Provider SLA 或业务正文审计。

## 6. 测试资产边界

- 独立 OpenAI SDK 的 Native Responses JSON/SSE 两轮工具回传已通过[固定版本 loopback 验收](evidence/2026-09-09-openai-responses-sdk-loopback.md)；真实 Provider、Bridge、并行工具及完整 Agent runtime 未由该 gate 验证。
- `forwarding_contract/resilience.rs` 的受控 producer 验证 SSE Body 按下游需求拉取、恢复消费和 drop 释放；带后台预读的负向控制会失败。该应用层回归不证明 TCP/HTTP2 背压、RSS 峰值或生产并发稳定性。


当前确定性测试和 corpus 的覆盖入口包括 registry、routing、wire、Generation Static/Event IR lifecycle、SSE fragmentation、retry/fallback/cooldown、取消，以及canonical wire case 经过 production Router 的目录驱动回放（`tests/catalog_replay_contract.rs`）；这些入口不等于当前运行结果，也不证明：

- 完整 Model/Provider inventory、retired ID 黑名单、完整 candidate 数量/顺序或每个 catalog capability fact；
- 每个 Provider/model 组合都重复经过 Native/Bridge production Router，或 OTLP metrics exporter 拥有独立进程级集成覆盖；
- stream-violation fixture 的 proposed oracle：当前首帧 event/type 冲突在 commit 前返回 502，已提交后的非法 lifecycle/arguments 终止 body；保留有效前缀，不合成替代终态。fixture 的其他 proposed 行为不构成待实施授权；
- canonical oracle 等于完整 OpenAI API；
- hosted/custom tool、continuation、媒体和 Provider 私有扩展可转换；
- 真实 SDK、Agent、Provider、TLS/HTTP2、并发背压、负载或真实 packet boundary 兼容；
- semantic reference trace、synthetic context byte/position sweep 或 strict JSON oracle 不证明真实 model 的 context limit、tokenizer、推理质量、Provider 原生 enforcement 或 OpenBridge production path 已执行；
- 外部来源未来保持相同行为。

尚未实施的测试切片（目录驱动回放之外的已知缺口）：

- 缓存管理没有独立 corpus case：`prompt_cache_key` 仅有候选投影/省略测试（`forwarding_contract/resilience.rs`）与 usage 解析测试（`observability_contract.rs`），没有 wire-level cache hint case，也没有登记对应 `sources/` 条目；
- `tests/semantic_router_contract.rs` 选择性消费 tool-result history、parallel arguments 与 structured output 的 canonical 数据，经过 production Router 并使用独立 JSON/SSE 投影；没有通用 semantic network runner，也不将所有模型任务扩为四方向验收。Native、reasoning、工具选择控制、普通用户/instruction 文本、usage 与媒体等完整语义矩阵不由该精简套件证明，继续由各自合同测试及本页未验证边界说明；


corpus 中未固定 source ref、pending license 与 `reviewed` case 必须继续显式暴露，不能改写为完成状态。实际执行过的外部证据以[evidence 索引](evidence/README.md)为准。
