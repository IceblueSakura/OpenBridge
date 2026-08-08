# Provider 实施与实测状态

本目录按 Provider family 汇总当前 checkout 中已经注册的 Public Model、模型特有多模态能力、工具调用能力，以及实际执行过的
确定性测试和真实 Provider 探测。它不替代 [`features/`](../features/) 中按功能划分的实现专题，也不重复
[`references/providers/`](../../references/providers/) 中的外部协议说明。

状态页使用以下证据术语：

- **真实 Provider 实测支持**：固定公共 endpoint 上的请求返回成功，并产生与能力相符的可验证结果；
- **确定性支持**：当前 checkout 的 registry、planning、mock transport 或 fixture test 已通过，但不代表真实上游；
- **未实现**：真实 Provider 已观察到该能力，但 OpenBridge 当前没有对应的 Public Model interface 或 preflight；
- **不支持**：模型 contract 未声明该能力，或真实请求没有产生该能力要求的结果；
- **未验证**：没有足够的正向或负向测试，不能从同系列模型外推。

HTTP 200、接受某个字段或静默忽略参数本身不构成能力支持。例如 function tool 只有返回非空函数名及参数的有效 tool call，才记为
工具调用支持。

## Provider 状态页

| Provider | 状态页 | 当前注册模型 | 最近真实探测 |
|---|---|---:|---|
| Alibaba Cloud Model Studio | [Bailian 状态](bailian.md) | 12 | 2026-08-08 |
| LongCat | [LongCat 2.0 状态](longcat.md) | 1 | 2026-08-08（high）；none 尚未真实复测 |
| Kimi CN | [Kimi K3 状态](kimi-cn.md) | 1 | 2026-08-08 |
| Xiaomi MiMo | [MiMo 多模态与工具调用状态](mimo.md) | 6 | 2026-08-08 |

## 维护边界

- 每个 Provider 只保留一个当前状态页；模型新增、移除或实测结论变化时原地更新，不保留历史快照。
- 表格必须区分真实 Provider、OpenBridge 确定性测试和端到端网关验收，不能互相替代。
- 不记录 credential、完整请求/响应、原始 Base64、Provider request ID 或敏感业务内容。
- 动态 model list、账号权限、配额和服务行为可能变化；始终以当前 checkout 与最近一次明确探测为准。
