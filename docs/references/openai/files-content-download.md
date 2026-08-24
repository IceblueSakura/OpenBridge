# OpenAI Files content download 调研

## 来源、范围与快照

本文只记录通过 file identity 下载 content bytes 的 operation。Metadata、create、delete 与模型 file input 不在本文定义。

- 官方来源：[Files API](https://developers.openai.com/api/reference/resources/files)
- 原始资料复核日期：2026-08-04；本次结构整理未重新在线复核 content media type、range 或 size behavior。

## 1. Binary response

content download 可能返回 binary bytes，而不是统一 JSON envelope。兼容实现必须保持 success status、`Content-Type`、body bytes、
body limit、cancellation 与 error response 的 media-type 分界。

## 2. Ownership 与资源控制

file id 仍绑定原账户/项目和 purpose。下载 bytes、临时文件、streaming/backpressure、malware/content policy 与清理需要独立资源控制；
不能把 content response 写入普通日志。

## 3. 证据边界

- metadata 可读不自动证明 content 可下载；
- 小文件 success 不证明大文件、range、取消、超时或 content-type 兼容；
- hosted file input 不等于 gateway 必须代理 content download。
