# OpenAI Uploads transaction 调研

## 来源、范围与快照

本文只记录 Upload create、multipart parts、complete 与 cancel/expiry 组成的一个分片事务。最终 File resource 的其他生命周期由 Files
文档维护。

- 官方来源：[Uploads API](https://developers.openai.com/api/reference/resources/uploads)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核容量、part limit 或 expiry。

## 1. Transaction stages

1. create upload，获得临时 upload id 与 expiry；
2. 上传一个或多个 binary parts，每个 part 拥有 identity/order；
3. complete 按指定 parts 组装最终 file；
4. cancel 或 expiry 结束未完成事务。

这些 operation 共享同一 upload transaction，不能把 upload id、part id 与最终 file id 混为一种 identity。

## 2. Request form 与 state

create/complete/cancel 使用各自 JSON/resource request，part upload 使用 multipart/binary body。每个 stage 的 method、media type、成功体与
错误形状必须保持；complete 的一次性/terminal 语义不能由普通 POST retry 规则覆盖。

## 3. Retry 与恢复

网络结果不确定时，应先依据 upload state 确认已接收 parts 或 complete 状态，不能无条件重新创建事务或重复 complete。part ordering、
总 bytes、expiry 和 cancellation 都属于可恢复状态机。

## 4. 证据边界

- 历史容量与 expiry 数值只能按快照日期理解；
- Upload completion 只产生 File resource，不证明 Vector Store processing 或 model input；
- mock transaction 不证明真实大文件、并发 parts、超时或清理。
