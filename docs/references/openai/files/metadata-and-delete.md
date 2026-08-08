# OpenAI Files metadata、list 与 delete 调研

## 来源、范围与快照

本文只记录 Files resource 的 list、metadata retrieve 与 delete operation。Multipart create 和 binary content download 分别由其他文档
维护。

- 官方来源：[Files API](https://developers.openai.com/api/reference/resources/files)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 pagination、status 或 retention。

## 1. List 与 metadata retrieve

list 返回当前权限边界内的 File resource 集合；retrieve 使用 opaque file id 获取 metadata。pagination、ordering、status 与 filter
必须按当期 schema 保持，不能由本地文件系统语义推断。

## 2. Delete

delete 是有副作用的 resource operation。不存在、无权限、已删除或仍被其他 resource 引用的行为必须按正式 contract 区分；不能按
普通 GET 的 retry 假设处理。

## 3. Identity 与数据边界

file id、filename、purpose、status 与时间信息可能敏感。caller 能猜到 id 不代表拥有 retrieve/delete 权限；opaque id 也不能跨账户或
Provider 迁移。

## 4. 证据边界

- metadata retrieve 不证明 content download；
- list/retrieve success 不证明 delete 或 resource cascade；
- mock resource table 不证明真实 retention、并发 delete 或权限实现。
