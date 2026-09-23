# 扩展语义、上下文与来源约束

来源与本次核对：2026-09-23 固定的 OpenAI/Codex 公共源码，见[同步基线](upstream-sync.md)；历史 scoped tool、provider metadata、reasoning signature 与 state recovery 经验见[主题综合](semantic-baseline.md)。未运行 Codex、登录 OAuth 或调用 Provider。字段/lifecycle/profile 变更时重核。

本页区分外部事实与设计约束；接受的 IR 所有权由 [semantic-ir](../architecture-v2/semantic-ir.md)维护。扩展不是裸 `extra_body` 或 `extra_headers` 透传口。

## 1. 扩展的三种用途

| 用途 | 示例 | 不能误归为 |
|---|---|---|
| 标准以外的任务能力 | Provider 特殊音频/视频 part、tool signature、特殊输出控制 | 标准字段的第二份拷贝 |
| 请求/会话上下文 | Codex logical session、thread/window、agent lineage | prompt 文本、全局万能 SessionId |
| 受限 opaque replay | Provider 签发 token、signature、turn-state | 可自由跨目标复制的用户 JSON |

OpenAI 已公开标准化的 `phase`、reasoning context、标准 hosted tools 必须进入标准分支。不是所有 `x-*`、body 私有字段都属于同一扩展；位置不是语义所有权。

## 2. Codex session_id 的实际含义

本次重新核对的源码仍显示以下区分：

| 事实 | Wire 投影与生命周期 | 基线约束 |
|---|---|---|
| Logical session | `client_metadata.session_id` / turn metadata | 真实会话来源提供；不以请求 UUID 或缓存 hash 冒充 |
| Cache affinity | root agent 的 `session-id` 通常来自 effective prompt cache key | 与 logical session 分开；non-root agent 的选择不同 |
| Thread | `thread-id`；当前 `x-client-request-id` 同样用 thread identity | 不解释为每 HTTP attempt 唯一 ID |
| Context window | window identity / `x-codex-window-id` | compaction 后可变化，不等于 thread |
| Turn sticky state | server-issued `x-codex-turn-state`，同 turn 重放 | 不可派生；新 turn 或 auth ownership 变化清除 |
| Canonical metadata | `client_metadata["x-codex-turn-metadata"]` | flat body keys 和 HTTP/WS headers 是同一事实的投影 |
| Credentials/account | Authorization、account/compliance headers | 不进入普通扩展；由安全 binding owner 在执行时处理 |

`prompt_cache_key()` 的优先级为显式 override、某些 internal session 的 source+parent-thread、logical session；root `responses_session_id()` 采用该 effective cache key，non-root 使用 logical session。该规则属于固定 Codex/ChatGPT profile，不是所有 OpenAI Responses 服务的默认合同。

HTTP header 与 WebSocket handshake/message 的位置不同；turn-state 还可能进入 WS message client metadata。扩展 schema 应描述事实与生命周期，再由 profile encoder 投影，不能同时保留多个可矛盾的 canonical 值。

## 3. 扩展准入要求

每个可编码扩展至少需要明确：

- namespace、kind、schema version 和固定来源；
- attachment owner：request context、item、part、resource 或 event；
- typed payload／明确 schema 的 bounded opaque value；
- origin 与有效 scope、生命周期、是否 final/replayable；
- 信任来源、上下游可见性、日志/隐私级别；
- 可表示目标与跨 profile/issuer 的映射或拒绝条件；
- 对 requirements、删除/替换、终态、重试/fallback 的影响。

这是一份编译期合同，不要求先建动态插件系统。没有已验证 codec/目标 scope 的扩展不得因名字匹配就转发；未知数据可作为有界不可执行诊断，但不能无条件进入未来请求。

## 4. 信任与数据流

标准 `metadata` 是业务数据，不是扩展注册表；任意字符串不能选择 route、endpoint、credential 或脚本。扩展 namespace 可以声明语义来源，但不能授予对该 Provider 的访问权。真实 source scope 必须由可信边界绑定，不能接受客户端声称“我是某 issuer”。

hosted MCP 标准 schema 含 server URL、headers 与 authorization 等敏感入口。支持它的语法不等于解除网关安全边界：未来执行仍需受信目标准入和独立 credential binding，秘密不能塞进可日志化 IR。远程媒体 URL 也不授予 codec 下载权限。

session/thread/cache/turn 值即使不是密码，也可能敏感且高基数；默认不作为 metrics label 或普通日志字段，不在文档/fixture 保存真实值。

## 5. 标准化升级与兼容

当某扩展被官方标准吸收：核对语义是否等价，迁入标准 owner，消除同一事实两份字段。旧 wire spelling 如仍需接受，由显式 profile codec 处理，不在 IR 保留 legacy alias。

当前 `reasoning.summary:false` 是本地接受的兼容形式，但 SDK `3.19.0` 和本次公开 reference 的标准 summary 是字符串枚举或 null；不能把现有行为自动写成标准。具体兼容 profile 的公开名和 downstream extension envelope 尚未制定，不以文档示例冒充已发布 API。

## 6. 需要单独定稿的事项

- downstream 扩展承载位置、namespace 命名与版本协商；
- Codex 上下文是由客户端可信 adapter 提供、透明转发，还是由未来 Gateway 管理 turn；当前不自动生成身份或 sticky token；
- 外部 opaque state 的可信 scope 构造、失效、principal 隔离；
- 特殊多模态的具体 Provider/operation/schema；没有固定事实不预造字段全集。

这些事项不妨碍确定 Responses-first 和分层扩展基线，但在实现相应 wire 接口或 state owner 前必须解决。
