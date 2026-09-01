# 当前状态边界

本文是当前 checkout **未实现、未验证和证据适用范围**的唯一实施状态 owner。它不构成路线图或实施授权；明确非目标仍由[产品范围需求](../functional-requirements/product-scope.md)拥有，下一项获准行为只由[当前开发焦点](../implementation-plans/current-focus.md)管理。

## 1. 如何解释这些边界

证据按以下层级分别记录，低层不能替代高层：

1. 静态源码、schema 或编译检查；
2. 确定性 Rust test 与 fixture；
3. Python corpus/testkit 或独立 loopback；
4. 外部 SDK 或独立 curl/Python 客户端；
5. 目标 Agent runtime；
6. 真实 Provider；
7. 负载、长期运行或生产环境。

“未证明”只表示当前证据没有覆盖，不等于已知不可行；“未实现”表示当前 checkout 没有对应 executable contract。带日期的外部结果只适用于记录中的版本、账号、区域、网络、模型和 payload，不提升为长期能力保证。

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
- Bailian Responses Session cache 只表示固定 header 已进入受信 egress；cache hit、节省成本、TTL、Provider 保留策略、突发排队命中率
  及其在真实 429 下的延迟改善均未验证。
- serial-only Provider 的 `parallel_tool_calls:false` 安全省略合同尚未注册；当前 active true/false 都只在固定 interface 已证明可控制并行调用时接受。
- Models/Target catalog 不能证明 credential 可用、网络可达、配额、账号 entitlement 或模型质量。

### Embeddings

- 当前只有单 Route Native execution；没有跨 Route fallback、Bridge、数值向量转换、缓存、索引或检索。`bailian/qwen3-7-text-embedding` 的 target/API-scoped float32/Base64
  wire re-encoding 只改变表示，不执行归一化、降维或模型转换。
- `qwen3.7-text-embedding` 的北京 OpenAI-compatible float、1024/512 维、20 条 batch 和基础中英排序已有带日期小样本；Hindsight
  SDK Base64 路径在升级前线上版本仍被 preflight 拒绝，修复后的部署态、完整 Hindsight runtime、语义 benchmark、生产配额、负载和长期网络可用性仍未验证。

### Native 图片与文件输入

- 图片 Bridge、Pro 图片、`file_id`/Files、image edit、Provider-side DNS/redirect/MIME/size、OCR、内容安全和显式 detail 未证明；Bailian Qwen 按官方共同上限公开 250 张 remote/inline BMP/JPEG/PNG/TIFF/WebP/HEIC，但 4K 以上仅 JPEG/PNG、remote 20 MB、宽高大于 10、200:1 宽高比和 8K 建议边界仍不能由本地 media 类型完整表达；Bailian Kimi 继续保持实测的单张 JPEG/PNG。
- OpenBridge 不下载、解析、转换、转码、缓存或扫描远程图片/文件。
- 当前生产 Public Model 不公开 file input；synthetic file loopback 不证明真实模型/backend、Provider 下载行为、解析质量、费用或 SDK/Agent 兼容。

### Native 音频

- OpenAI `/v1/audio/*`、Responses audio、Realtime、remote/multiple audio、更多格式/语言/voice、媒体质量、voice authorization/store 和跨请求 voice identity 未证明。
- 五种 MiMo 音频 task 的真实下游网关复测、播放器/硬件验收、负载和长期运行未完成。

### Images Generations

- I2I/edit/variation、异步任务轮询、stream 输出和 `b64_json` 未实现。
- Images 当前不复用 Generation/Embeddings recovery runner；单请求可能已计费，因此没有 retry、fallback 或 credential rotation。
- 图像 URL 是 Provider 返回的临时签名 URL；OpenBridge 不下载、缓存或延长有效期。
- 真实 OpenAI Images SDK、图像内容质量、计费语义、配额、内存峰值、SLA、负载、长期取消和生产 logging 未验证。

### MCP

- 当前本地 tool 只有 `hello`；没有 hosted tool、MCP Tool Bridge 或由 generation gateway 执行普通 function tool。
- 进程内 MCP contract 不证明外部 MCP SDK、浏览器、反向代理、工具安全、真实网络部署、负载或长期运行。

## 4. Provider 特定边界

- 2026-08-31 的有界管理员 probe 覆盖 DeepSeek、MiMo 与 GLM Chat，并覆盖 Bailian DeepSeek V4 Flash 和 Zhipu GLM-5.3 Responses JSON/SSE；除 2026-09-01 单独完成的 MiMo-V2.5 Chat JSON Object JSON/SSE Gateway 验收外，不证明其他下游 Gateway 端到端、reasoning 参数实际生效、工具/媒体、structured output、fallback、SDK/Agent 兼容、负载、长时间运行或未来可达性。

| Provider family | 当前未实现或未证明边界 |
|---|---|
| ChatGPT | WebSocket、Batch、Embeddings、hosted/custom tool、MCP、真实图片输入、background/stateful response、完整 Agent loop、多账户轮换、外部 SDK、负载和长期 refresh 稳定性。 |
| OpenAI | 当前没有成功的真实账号/Provider 验证；Models、Chat/Responses、Embeddings、图片、strict/parallel tool、structured output、state、配额、负载和长期运行均不能由静态 ceiling 推断。 |
| LongCat | 更多 reasoning 档位和 tool 形状、强制 Bridge/fallback、外部 SDK/Agent、负载与长期运行。 |
| DeepSeek | Vision Files API/`file_id`、role/像素边界本地预检、600 图与 remote/mixed 64 MiB 极限、任意 remote host 可下载性、恶意图片、视觉质量与更高阶多能力组合未证明；inline executable profile 保守限制为累计 decoded 32 MiB。Pro 已记录官方 `low/high/max` 档位；普通 endpoint 不公开仅 `/beta` 保证的 function strict schema。`parallel_tool_calls` 请求控制、hosted/custom tool、structured-output SSE、强制 fallback、其他账号/区域和长期运行仍未证明。 |
| Xiaomi MiMo | 独立 Chat structured-output 专页确认 MiMo-V2.5/Pro 的 JSON Object，但 Chat API reference 同时只列 text；当前只保留专页明确的 Chat JSON Object。2026-09-01 已对 MiMo-V2.5 以固定 conflict prompt 完成管理员 probe，并以明确请求 JSON 的固定 prompt 完成真实下游 Gateway JSON/SSE 验收；这不证明 MiMo-V2.5-Pro、其他 schema/prompt、Responses structured output 或长期稳定性。Responses structured output、prompt-cache/include 不公开；video、remote/multiple audio、更多媒体格式和 limit、parallel 稳定性、ASR 方言质量、TTS 音质、外部 SDK/Agent、负载与长期运行未证明。 |
| Zhipu AI China | GLM-5.3、GLM-5.2 与 GLM-5.3-Flash 注册 Chat Native JSON Object；Chat function tools 只公开 Auto choice，不公开请求级 parallel 或 strict schema。只有官方明确列出的 GLM-5.3 注册 `/api/v1/responses` text-only Native，并保留 Chat bridge 作为更高能力请求的回退。2026-08-31 的有界 JSON/SSE Responses probe 均成功；Responses reasoning、structured output、工具、state，以及文件、视频、外部 SDK/Agent、其他账号/区域、负载与长期运行未证明。 |
| OpenRouter | MiniMax 图片输入没有模型级真实 Provider 证据；Gemma 的历史 probe 只证明单张 PNG data URL，不能支撑共享 profile 的 JPEG、remote URL、4-part 与大小上限；两者当前 executable interface 均保持 text-only。Muse Spark 1.2 Contributor 的文本 Chat/Responses 与 Hermes `obc`/`obr` 已真实 probe；图片、音频、视频与文件输入尚未 probe，当前 executable interface 保持 text-only。GLM-5.3-Flash 已验证 Chat/Responses streaming、non-streaming、PNG data URL、Auto function tool、parallel 请求开关与 Hermes `obc`/`obr`；named tool choice 和 Responses structured output 经 probe 后显式不公开。GLM 的 OpenRouter file input、remote image/JPEG、video、更多图片数量/大小、长上下文、负载和长期运行仍未证明。Gemini/Grok file/audio/video、Grok 小图尺寸边界、更多图片格式/数量/大小、强制 DeepSeek fallback、Gemma reasoning、MiniMax/NVIDIA failover、Provider routing 偏好、外部 SDK/Agent、负载与长期运行未证明；公开目录字段不自动成为 executable capability。 |
| NVIDIA | MiniMax 强制 fallback、图片/tool/structured output、真实 reasoning、Embeddings 语义质量、其他账号/区域、配额、负载与长期运行。 |
| Alibaba Cloud Model Studio | 2026-08-27 北京真实 Responses 对比确认基础 JSON/SSE、usage 和第一轮工具 wire 可用；在统一冲突提示下，三模型均忽略 Responses `text.format=json_object/json_schema`，且在单一双调用提示下均未执行 `parallel_tool_calls=false`。GLM-5.2 另有高 reasoning 400 与标准工具续轮 arguments 类型冲突，当前继续只走 Chat bridge。Qwen3.8 Max、DeepSeek V4 Pro 0813 与 DeepSeek V4 Flash 0731 已注册双 Native；2026-08-31 再次确认 Flash 的有界 Responses JSON/SSE，但未执行 production Router 或 SDK/Agent。Bailian Chat structured output 只按官方模型范围公开，Responses structured output 继续按既有差分收窄。Qwen/DeepSeek Responses Target 的 `parallel_calls=false` 表示不公开可精确执行 true/false 的控制，不是 serial-only 保证。LiveTranslate 没有下游 executable interface；Images I2I/async/stream/`b64_json` 未实现；Qwen/Kimi video、多图、更多图片格式/尺寸/detail、多模态 tool 组合、强制 DeepSeek fallback、其他账号/区域、质量、计费、负载与长期运行未证明。 |
| Kimi CN | 其他 Moonshot endpoint、原生 Responses、更多参数组合、账号权限、外部 SDK/Agent、负载与长期运行。历史 `none` 结果不证明当前可关闭 reasoning。 |

Provider 外部观察见[evidence](evidence/README.md)；动态官方文档和模型目录见[references](../references/README.md)。

## 5. 观测、配置与生产边界

- 确定性配置测试不证明 credential 有效、Provider 可达、OAuth authority/refresh 长期稳定、collector/sink 可用或生产日志保留策略正确。
- 当前没有 OTLP logs、内置 Prometheus、dashboard/告警、metrics 历史数据库或多进程聚合。
- 本地 JSONL content snapshot 是受控开发能力；没有生产敏感流量、资源开销、磁盘故障、负载或长期运行验收。
- 当前没有真实 Provider wire dump；普通 telemetry 不用于计费准确性、Provider SLA 或业务正文审计。

## 6. 测试资产边界

当前确定性测试和 corpus 能证明 registry、routing、wire、Generation Static/Event IR lifecycle、SSE fragmentation、retry/fallback/cooldown、取消和有限 replay，但不证明：

- 完整 Model/Provider inventory、retired ID 黑名单、完整 candidate 数量/顺序或每个 catalog capability fact；
- 每个 Provider/model 组合都重复经过 Native/Bridge production Router，或 OTLP metrics exporter 拥有独立进程级集成覆盖；
- 全部 canonical case 都经过 production Router；
- canonical oracle 等于完整 OpenAI API；
- hosted/custom tool、continuation、媒体和 Provider 私有扩展可转换；
- 真实 SDK、Agent、Provider、TLS/HTTP2、并发背压、负载或真实 packet boundary 兼容；
- semantic reference trace、synthetic context byte/position sweep 或 strict JSON oracle 不证明真实 model 的 context limit、tokenizer、推理质量、Provider 原生 enforcement 或 OpenBridge production path 已执行；
- 外部来源未来保持相同行为。

corpus 中未固定 source ref、pending license 与 `reviewed` case 必须继续显式暴露，不能改写为完成状态。实际执行过的外部证据以[evidence 索引](evidence/README.md)为准。
