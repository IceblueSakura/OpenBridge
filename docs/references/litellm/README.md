# LiteLLM 调研索引

本目录记录 LiteLLM Proxy/SDK的协议转换、请求链、observability、retry、OAuth、server-tool interception与测试资产。许可证见
[MIT；enterprise subtree另有条款](https://github.com/BerriAI/litellm/blob/main/LICENSE)。本轮只读拉取和源码复核没有安装依赖、运行LiteLLM或调用Provider。

最新IR/server-tool增量复核基线为 `litellm_internal_staging` @
`5e4b3838aabf00d135be800404d03728c8afa506`；其他叶文档保留各自更早的逐行快照、精确行号和复核日期。

| 主题 | 文档 |
|---|---|
| Chat/Responses 双向转换 | [Chat Completions 与 Responses](litellm-chat-responses-analysis.md) |
| Proxy 请求路径 | [Proxy 调用链](litellm-proxy-call-chain-analysis.md) |
| 性能假设与瓶颈 | [Proxy 性能瓶颈](litellm-proxy-performance-bottlenecks.md) |
| Metrics 与 TTFT | [调用统计与 Prometheus](litellm-observability-analysis.md) |
| Deployment retry/cooldown | [credential pool 与 retry](litellm-credential-pool-retry-analysis.md) |
| ChatGPT credential lifecycle | [ChatGPT authenticator](litellm-chatgpt-oauth-refresh-analysis.md) |
| Protocol regression assets | [Responses与转换测试资产](litellm-protocol-test-assets-analysis.md) |
| IR、state与server-tool增量 | [Responses bridge与server-tool regressions](litellm-ir-server-tool-regressions-analysis.md) |

LiteLLM 的 deployment、team、budget、virtual key、cache 和转换 fallback 是其产品控制面，不自动形成通用协议。升级 LiteLLM，
依赖精确调用链/metrics 口径，或采用某个转换与 retry 行为前，必须回到叶文档的固定 commit 重新核对；静态源码也不证明真实
Provider、负载或生产部署表现。
