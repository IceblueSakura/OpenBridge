# OpenAI Responses File Search 调研

## 来源、范围与快照

本文只记录 Responses hosted File Search tool 的 resource reference、call/result 与 citation 边界。File input、Files API 和 Vector Store
lifecycle 由各自文档维护。

- 官方来源：[File search](https://developers.openai.com/api/docs/guides/tools-file-search)、[Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 tool schema、include enum 或 model capability。

## 1. Hosted tool request

File Search 由服务执行，并引用 vector-store identity。tool declaration、resource ids、search options 与 model capability 共同决定行为；
它不能降级为 client function tool。

## 2. Result 与 citation

call/result item、可选 include 数据、citation/annotation 与最终 message output 是不同结构。`output_text` 不能替代完整 tool item 和
引用关系；streaming 时 item done 也不等于 response terminal。

## 3. Ownership 与成本

tool 只能访问授权 resource。Vector Store processing、file permissions、检索/存储成本和数据 retention 不由一次 Responses request
隐式解决。

## 4. 证据边界

- File Search success 不证明 Files upload、Vector Store processing 或普通 `input_file` 兼容；
- 一个 query 不证明 citation completeness、ranking、错误或 streaming event；
- mock result 不证明真实 hosted retrieval、权限或费用。
