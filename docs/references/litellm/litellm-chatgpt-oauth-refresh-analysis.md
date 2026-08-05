# LiteLLM ChatGPT authenticator 调研

## 状态与证据

- 原始逐行证据快照：`BerriAI/litellm` commit `bd44c9e305b89526d4c5d773ee39ca935561b9c8`
- 当前模块级复核：commit `23de7a15d9d40006ee596e617475ba101d60c5e9`，2026-08-05
- 阅读范围：`litellm/llms/chatgpt/authenticator.py` 与 Chat/Responses transformation
- 未读取、输出或复制本地 credential、token、client identifier 或 auth file 内容。

## 1. auth file 与 token resolution

LiteLLM `Authenticator` 默认使用一个本地 JSON auth file，并允许通过环境变量改变目录或文件路径。`get_access_token()` 的顺序为：

```text
read auth file
  -> token valid outside 60-second skew: return
  -> refresh token exists: refresh and write file
  -> otherwise or refresh failed: enter/wait for device login
```

expiry 优先读取 `expires_at`，缺失时从 access token JWT `exp` 推导。该实现主要在 token 被请求时检查，不包含后台 scheduler。

## 2. 设备登录与 refresh

[
`authenticator.py`](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/llms/chatgpt/authenticator.py)
请求 Codex 私有 device user code，显示 verification URL/code，轮询 authorization code 与 PKCE verifier，再执行
authorization-code exchange。最长等待 15 分钟。

refresh 使用 `grant_type=refresh_token`。若响应含新 refresh token 就替换，否则保留当前值，随后写回同一 JSON 文件。

## 3. account 与请求 header

account ID 优先来自 auth record；缺失时从 id/access token claim 推导并写回。Chat 与 Responses transformation 在
validation/token-resolution 路径取得 access token 与 account context，再构造 bearer、account、originator、User-Agent 和
session header。

Responses transform 还强制部分 Codex-shaped 请求选项。这些行为属于 LiteLLM 的 ChatGPT provider adapter，而不是普通 OpenAI
API-key adapter。

## 4. 并发与持久化边界

固定快照中的 `_read_auth_file()` 和 `_write_auth_file()` 使用普通读取及 `json.dump()` 覆盖写。在该 authenticator 范围内未观察到：

- file lock 或 process-wide lock；
- temporary file + atomic replace；
- credential version/CAS；
- 跨 worker refresh single-flight；
- 多账号 isolation。

因此并发调用可能同时看到过期 token 并执行 refresh。若 authorization server 使用 single-use rotation，最后写入者不能自动保证保存了有效
token pair。

## 5. 适用边界

- 在 token resolution 内回退到交互式登录是该 adapter 的产品行为。
- 单 JSON 文件实现不能说明多进程 credential manager 的安全性质。
- LiteLLM 其他 MCP OAuth store 服务于不同功能，不能与 ChatGPT provider credential 混用证据。
- 私有 endpoint 与 header 的复现不构成 OAuth client 使用授权。

## 一手源码

- [
  `authenticator.py`](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/llms/chatgpt/authenticator.py)

