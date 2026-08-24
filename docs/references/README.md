# 参考文档

本目录只保存外部协议、SDK、目标客户端、Provider 和参考项目的固定调研。外部事实以各叶文档记录的 URL、日期、版本或
commit 为准；目录索引只负责导航和维护规则，不用较新的索引日期覆盖原始快照。

本目录不记录 OpenBridge 当前实现、源码结构、已执行测试或实施方案。产品合同、当前事实和唯一获准计划分别由
[functional requirements](../functional-requirements/README.md)、[implementation status](../implementation-status/README.md)
和 [current focus](../implementation-plans/current-focus.md) 维护。

## 1. 分类入口

| 类别 | 入口 | 内容 |
|---|---|---|
| OpenAI 协议与 SDK | [OpenAI 调研索引](openai/README.md) | API operation、SDK consumer、gpt-oss 与 Open Responses 测试资产 |
| Provider | [Provider 调研索引](providers/README.md) | 各上游 API、认证、固定 wire 与专项媒体观察 |
| MCP | [MCP Rust 生态索引](mcp/README.md) | MCP 规范、远程访问模式与 Rust SDK |
| 参考项目 | [参考项目总览](project-comparison.md) | 项目证据角色、互证关系与局限 |
| Codex | [Codex 调研索引](codex/README.md) | SSE、tool lifecycle、OAuth 与测试资产 |
| Hermes Agent | [Hermes 调研索引](hermes/README.md) | Chat/Responses consumer、credential lifecycle 与插件能力 |
| LiteLLM | [LiteLLM 调研索引](litellm/README.md) | Proxy、转换、observability、retry 与 OAuth |
| new-api | [new-api 调研索引](new-api/README.md) | 多协议转换、渠道路由、计费与运维机制 |
| cc-switch | [Chat/Responses tool conversion](cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)、[retry/failover](cc-switch/cc-switch-retry-failover-analysis.md) | 桌面客户端 bridge 与 Provider failover |
| CLIProxyAPI | [stateful bridge](cliproxyapi/cliproxyapi-stateful-bridge-analysis.md)、[credential retry](cliproxyapi/cliproxyapi-credential-pool-retry-analysis.md)、[OAuth scheduler](cliproxyapi/cliproxyapi-codex-oauth-refresh-analysis.md) | 订阅账号代理的 state、cooldown 与 OAuth lifecycle |
| 跨项目综合 | [综合调研索引](cross-project/README.md) | 只汇总已经存在项目级前置文档的比较 |

语音资料按证据所有权分开：标准 Audio/Speech、Chat audio 与 Realtime wire 见
[OpenAI 音频与语音索引](openai/README.md#6-音频与语音)；MiMo wire 见
[Xiaomi MiMo 语音协议](providers/xiaomi-audio.md)。模型具备音频能力不等于兼容某个标准 endpoint。

## 2. 叶文档元数据合同

新建或实质更新的研究叶文档必须明确维护下列五项。可以使用表格、列表或等价小节；目录 README 可以聚合导航，不必复制每个叶文档的元数据。

| 字段 | 必须回答的问题 |
|---|---|
| Source snapshot | 使用了哪些官方 URL、仓库 commit、版本或脱敏响应快照？ |
| Last reverified | 最后核对的是外部来源、固定本地 checkout，还是仅本文综合与链接？日期是什么？ |
| Scope | 本文实际阅读、请求或比较了哪些 endpoint、模块、模型或场景？ |
| Evidence boundary | 这些证据不能证明哪些 Provider、账户、SDK、负载、长期运行或产品实现事实？ |
| Recheck trigger | 哪些 SDK/Provider/协议/目录变化，或哪类采用决定，会要求重新固定证据？ |

“Last reverified”不得把本地链接检查或文档整理写成外部实时复核。动态网页、模型目录和真实请求必须保留采集日期；固定源码结论必须保留
commit。真实观察还应说明账户、网络、payload 与敏感数据边界，不保存 credential、Authorization 值或未脱敏 production transcript。

## 3. 所有权规则

1. 单一项目或 Provider 的事实先进入对应目录；跨项目结论只进入 [cross-project](cross-project/README.md)，并链接全部项目级前置。
2. 外部目录字段、endpoint 供应状态与一次真实请求是不同证据层，不能互相替代。
3. OpenBridge 需求、当前代码、配置、测试结果和目标数据类型不写入参考叶文档；需要比较时只保留中性的采用边界。
4. 原始 JSON 等非 Markdown 资产必须有一个明确的 Markdown owner，记录采集、脱敏、大小/校验或复核边界。
5. 不为只有少量叶文档的目录机械增加 README；本页直接导航 cc-switch 与 CLIProxyAPI。
6. 对 official website 或 OpenRouter 可直接取得的模型信息，只记录来源 URL、来源身份、`Last reverified` 与 `Recheck trigger`；不保存完整 capability metadata、字段表、价格表、Provider 全量 Models 响应或原始 payload。
7. 当前 Model↔Provider 关系由 implementation status 维护，能力字段回到代码、运行中的扩展 Models API 或外部官方文档。只有执行测试与引用来源矛盾时，才由 implementation evidence 单独记录来源声明和观察差异；来源之间的静态字段差异本身不构成测试证据。

## 4. 固定项目基线

下表只汇总已经记录的本地 checkout 复核位置，不刷新叶文档的原始逐行快照。许可证以各项目仓库根文件为准，此处不是法律意见。

| 项目 | 许可证 | 汇总复核位置 | 已复核主题 |
|---|---|---|---|
| Codex | [Apache-2.0](https://github.com/openai/codex/blob/main/LICENSE) | `main` @ `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff` | device/browser auth、refresh、Responses SSE/tool tests |
| Hermes Agent | [MIT](https://github.com/NousResearch/hermes-agent/blob/main/LICENSE) | `main` @ `a31be48030f60383bf4c1d96ba46bd4b48430218` | Chat/Responses mode 与上游请求；credential lifecycle 见专项快照 |
| LiteLLM | [MIT；enterprise subtree 另有条款](https://github.com/BerriAI/litellm/blob/main/LICENSE) | `litellm_internal_staging` @ `23de7a15d9d40006ee596e617475ba101d60c5e9` | Responses routes、ChatGPT authenticator、metrics modules |
| new-api | [GNU AGPL v3](https://github.com/QuantumNous/new-api/blob/2d8e50bf36e94200b809dfb39e73624ec48b1e23/LICENSE) | `main` @ `2d8e50bf36e94200b809dfb39e73624ec48b1e23` | 请求主链、converter registry、渠道路由、计费与后台任务 |
| cc-switch | [MIT](https://github.com/farion1231/cc-switch/blob/main/LICENSE) | `main` @ `ebbf141fc71547a99f669df1be8e345130d1d890` | bridge state、history、retry/failover |
| CLIProxyAPI | [MIT](https://github.com/router-for-me/CLIProxyAPI/blob/main/LICENSE) | `main` @ `bc71c77f5cc42f3fbe1bf040cf14d4f166894835` | stateful translator、credential retry、OAuth scheduler |

## 5. 维护检查

- 相对链接、锚点和非 Markdown 资产 owner 可达；
- source URL、snapshot date/commit、阅读范围与复核触发条件完整；
- 观察事实、推论、未知项和采用边界分开；
- 综合文档链接全部项目级前置，不在综合页首次引入项目事实；
- 动态官方事实在升级兼容结论前重新固定，不把目录、SDK 或一次请求提升为长期保证；
- 不包含 credential、私有配置、敏感请求正文或未脱敏 transcript。
