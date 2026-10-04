# 协议来源入口

这里只维护仍用于设计的标准出处、固定版本和必要许可。接受的决策归 [ADRs](../architecture-v2/README.md#架构决策)，实现细节归代码和邻近注释；不保留历史分析、项目比较或测试报告。

| 入口 | 用途 |
|---|---|
| [固定上游来源](upstream-sync.md) | OpenAI SDK、Codex 的固定提交、许可和官方页面入口 |
| [Responses 标准基线](responses-standard.md) | Responses codec 的固定公开语义；不是共享 IR 的上限，也不等于本地准入 |
| [扩展与上下文](extensions-and-context.md) | session/cache/turn、存储与连接状态；固定 Codex/pi 投影，与认证 owner 分开 |
| [账户登录来源与采用边界](oauth-login.md) | 共用 OAuth/OIDC 标准、授权边界与必要归档导航 |
| [Grok Build / xAI 登录](grok-login.md) | 官方浏览器/标准设备授权、pi 内置与补充参考、credential/backend 边界 |
| [ChatGPT 登录：Codex 与 pi](chatgpt-login.md) | Codex 产品流程与公开 SIWC 动态 registration；身份、账户、refresh 和推理隔离 |
| [多模态与资源](multimodal-and-resources.md) | task、wire、资源与媒体的语义边界 |
| [Codec 验收方法](conformance-baseline.md) | 独立 oracle、变换和失败/资源边界；不是执行记录 |
| [OpenAI operation 导航](openai/README.md) | 集中定位标准资料与既有日期；Embedding 有独立任务来源，不复制多份字段/事件快照 |
| [Provider 官方入口](providers/README.md) | 包括 Google Gemini、Anthropic Messages 的一手设计参照；是查询导航，不是兼容清单 |

IR 的共享语义由[设计基线](../architecture-v2/semantic-ir.md)定义，不由任一参考协议独占。现有 OpenAI SDK/Codex 固定版本保持原有用途；新增协议前另行固定所选 operation、API/schema/SDK 版本与许可，不从动态网页推定稳定合同。

新增或修改协议行为时，按需核对一手来源，将必要 URL 和非显然理由留在 owning code；只有跨模块决策才更新 ADR。固定标准、SDK consumer 和产品私有协议不能混为同一合同。不要为每次调查新增分析页，也不要把本地实现缺口写成标准限制。

使用外部资产前核对具体版本、许可、敏感性和独立预期；默认自主编写最小 synthetic fixture，不复制真实会话。实际 Provider/模型准入按 [AGENTS 查询流程](../../AGENTS.md#current-provider-model-and-compatibility-information)确认，历史成功和源码类型均不能替代现场验证。
