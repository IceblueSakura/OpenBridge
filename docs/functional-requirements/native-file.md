# Native 文件能力需求

## 范围

本页只定义 Chat `file` 与 Responses `input_file` content part 的同协议 Native 输入能力。它不定义 Files/Uploads/Vector Stores/File
Search 生命周期；共同规则见[媒体扩展共同规则](embedding-and-native-multimodal.md)。当前尚无已完成的 Native file 功能专题。

## 1. 用户结果与 source 规则

| 协议 part              | 可建模来源                             | 首个目标边界                                                                                    |
|------------------------|----------------------------------------|-------------------------------------------------------------------------------------------------|
| Chat `file`            | `inline_data`、`file_id`               | 只开放 profile 声明的 inline data；不虚构 `file_url`、MIME 字段或 `detail`                      |
| Responses `input_file` | `inline_data`、`remote_url`、`file_id` | 开放 profile 声明的 inline/URL；`detail` 只对明确文件类别生效；无 resource affinity 时拒绝 ID |

每个 part 必须满足协议的一选一 source 规则。Responses `input_file` 不能同时携带 `file_data`、`file_url` 与 `file_id`；Chat
`file` 不能接受标准 wire 中不存在的 URL/MIME/detail 字段。inline file 必须保留并校验 filename、profile 声明的
`raw_base64`/`data_url` encoding，以及 wire 实际携带的 media type 语义。

## 2. `multimodal_input.file` 公共契约

文件子契约至少明确：

- 允许的 inline/remote source 与 inline encoding；
- 可验证 media type、filename 和适用文件类别；
- `detail` default/allowed domain 及其适用类别；
- part 数、URL 长度和单项/累计 inline encoded/decoded byte 上限。

任一必需集合为空、default 不一致或某个 Route 无法保证该 source 时，对应接口不得公开文件能力。嵌套字段不加入顶层
`supported_parameters`。

## 3. Resource identity、预检与保真

- OpenBridge 尚无 Provider/Target resource issuer、owner 与 continuation affinity 时，必须在首次 egress 前拒绝 `file_id`。
- remote URL 服从有界 absolute HTTPS 语法策略；OpenBridge 不下载文件，也不能证明 Provider-side DNS、redirect、MIME 或大小。
- inline data 必须在大分配前完成 encoding、media type、filename 和字节上限检查；请求分析冻结实际 source/encoding/detail/limit
  facts。
- Native 转发保持 filename、inline data、URL、detail、part 顺序与原协议 terminal；不得提取文本、转换格式、缓存或签发新 ID。
- Bridged Route 对 file source 贡献空集；不能根据本次 file source 跳过 Route 或执行多模态 Bridge。

## 4. 重放与数据保护

- 超过 replay budget 但仍在 request hard limit 内的合法文件请求只执行第一次 attempt。
- 首个业务输出后不得 retry/fallback；下游取消必须停止发送/接收和 backoff。
- URL query、filename、file ID、原始文件、Base64、完整响应与解析错误上下文不得进入普通日志或 metrics label。

## 5. 验收

| ID      | 应被保护的可观察行为                                                                                                            |
|---------|---------------------------------------------------------------------------------------------------------------------------------|
| FILE-01 | Chat/Responses 各自只接受标准 content part、source one-of、encoding、filename/detail 与 typed limit。                          |
| FILE-02 | 无 issuer/owner affinity 时 `file_id` 在 egress 前稳定拒绝；不会跨 Provider/Target 猜测或迁移资源。                            |
| FILE-03 | Native wire、part 顺序和 metadata 保持；请求不进入 Bridge、下载、文本提取、转换或请求期能力路由。                              |
| FILE-04 | URL/inline limit、日志脱敏、replay budget、取消和首输出 commit 均有确定性测试；真实 Provider/SDK 层单独记录。                 |

## 6. 非目标与参考

非目标包括 Files lifecycle、Uploads、Vector Stores、File Search、资源 ledger、跨 Provider migration、媒体托管与通用安全扫描。

- [OpenAI Chat 文件输入调研](../references/openai/files/chat-input.md)
- [OpenAI Responses 文件输入调研](../references/openai/files/responses-input.md)
