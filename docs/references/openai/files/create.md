# OpenAI Files Create 调研

## 来源、范围与快照

本文只记录 Files API create/upload 的 multipart request 与创建出的 File resource metadata。List、retrieve、delete 与 content download
分别由其他文档维护。

- 官方来源：[Create file](https://developers.openai.com/api/reference/resources/files/methods/create)、[Files API](https://developers.openai.com/api/reference/resources/files)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 purpose、size limit 或 retention。

## 1. Multipart request

create 使用 multipart form，核心 parts 包含 binary `file` 与 `purpose`。filename、part content type、bytes 与 purpose enum 都属于
wire contract；SDK file helper 不能替代 multipart schema。

## 2. Created resource

success 返回 opaque file id 及 filename、bytes/size、purpose、status/time 等 metadata。字段与状态以当期 API Reference 为准。

file id 的可用 endpoint、权限与生命周期由签发账户/项目拥有，不能仅根据字符串形状跨 Provider 使用。

## 3. Resource 与重放边界

上传临时文件、body limit、清理、content policy 与 malware scanning 是独立资源控制。create 可能产生存储与费用；transport 结果
不确定时不能无条件重放。

## 4. 证据边界

- create success 不证明 content download、delete、input part 或 Vector Store processing；
- 一个 purpose/format sample 不证明全部 enum、size 或 retention；
- mock multipart 不证明真实存储或 policy behavior。
