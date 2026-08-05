# OpenAI Files API 协议调研

## 1. Resource surface

Files API 是资源生命周期，不是一次模型生成调用。官方 surface 包括 create/upload、list、retrieve metadata、retrieve content 与
delete。

资料：[Files API](https://developers.openai.com/api/reference/resources/files)、[Create file](https://developers.openai.com/api/reference/resources/files/methods/create)、[File inputs](https://developers.openai.com/api/docs/guides/file-inputs)。

## 2. Create 与 identity

- create 使用 multipart form，包含 binary file 和 `purpose`；
- response 返回 opaque file id、purpose、size、filename、状态/时间等 metadata；
- purpose 枚举、单文件上限、项目总存储和 retention 会变化；
- file id 的可用 endpoint 与生命周期受签发账户/项目约束。

## 3. Retrieve/delete/content

metadata retrieve、content download 与 delete 是不同 method。delete 不是普通 GET/POST 重试场景；content response 也可能是
binary，不是统一 JSON。

## 4. 边界

- opaque file id 不能假定跨账户、Provider 或 region 可用。
- caller 能猜到 id 不代表拥有读取/删除权限。
- multipart 临时文件、下载 bytes、malware/content policy 与清理需要独立资源控制。
- Files API 与 Chat/Responses inline file part、Uploads transaction、Vector Store membership 是相关但不同资源。

