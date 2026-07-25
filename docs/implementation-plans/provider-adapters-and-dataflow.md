# Rust Provider adapter 与数据流架构

## 状态

**代码注册表基线已实现；异构 Provider 尚未验证。** 当前已实现单一 OpenAI adapter、显式注册表、
共享 HTTP transport、SSE framing、原生 Chat/Responses pipeline、capability gate 和输出前 fallback。

## 1. 方向

系统采用：

> 编译期闭合 Provider dispatch + 每 Provider 独立实现文件 + 启动时 typed registry builder

- Provider 行为和部署定义均由 Rust 代码实现；
- 不通过配置加载 Provider、endpoint、模型、capability 或转换规则；
- 下游请求不能指定上游 URL、credential、认证 header 或 transform；
- bootstrap 只控制进程资源策略；
- 注册表构建后不可变，变更需要重新编译和重启。

## 2. 模块边界

```text
src/provider/
  contracts.rs      # adapter traits、安全 header、错误与事件类型
  credential.rs     # secret source 与短时 lease
  mod.rs            # ProviderKind、descriptor 类型、闭合 enum dispatch

src/providers/
  mod.rs            # 唯一显式注册入口
  openai.rs         # OpenAI descriptor、adapter、模型和 deployment 定义

src/registry/
  mod.rs            # definition、builder、校验与 immutable snapshot
```

`provider` 不包含具体 Provider 的字段转换；`providers/<name>.rs` 不决定 public alias 顺序以外的动态
路由；`registry` 不执行网络 I/O；`pipeline` 不识别 Provider 名称。

## 3. Provider 文件职责

每个 Provider 文件至少包含：

| 部分 | 职责 |
|---|---|
| `ProviderDescriptor` | capability 上界、endpoint profile、credential kind |
| `ProviderDefinition` | Provider id 与 credential binding |
| `ModelDefinition[]` | 模型 id、上下文、参数、reasoning 与 level |
| `DeploymentDefinition[]` | endpoint、真实 model id、timeout、能力收窄 |
| `RequestAdapter` | path、upstream model 和字段转换 |
| `HeaderAdapter` / `AuthAdapter` | 安全 header 与认证 |
| `ResponseAdapter` | SSE/响应终态 |
| `ErrorAdapter` | 错误分类与 retry hint |
| `CapabilityAdapter` | adapter 上界校验 |
| discovery request | 固定上游模型列表/能力探测请求 |

复杂转换必须写成 Rust 逻辑和 fixture，不能演变成通用 map/template DSL。

## 4. Dispatch

Provider 集合保持闭合：

```rust
pub enum ProviderKind {
    OpenAi,
}

pub enum ProviderAdapter {
    OpenAi(openai::OpenAiAdapter),
}
```

每个请求只在选择 candidate 后进行一次 enum dispatch。当前不需要 trait-object registry、
动态库或按 token/event 的字符串查找。

## 5. Native dataflow

```text
Inbound request bytes
→ ValidatedRequest
→ request feature classification
→ immutable RegistrySnapshot
→ eligible deployment candidate
→ ProviderAdapter::encode_request(request, upstream_model)
→ UpstreamRequestParts(relative URI, headers, body)
→ shared transport + registered endpoint base
→ upstream JSON/SSE
→ Provider response/error adapter
→ downstream native response
```

Pipeline 保留原始 JSON。OpenAI adapter 当前只写入实际 `upstream_model`，其他未知合法字段保持不变。

## 6. Bridge dataflow

Protocol Bridge 尚未实现。未来只在协议不一致时进入：

```text
source wire parser
→ BridgeRequest
→ target Provider adapter
→ upstream response/event
→ request-scoped assembler
→ source wire renderer
```

Bridge 能力不能仅靠注册项声明；必须有实现和 fixture 证据。

## 7. Transport 与安全

- adapter 只能生成无 scheme/authority 的相对 URI；
- endpoint base 只来自已校验代码注册项；
- transport 显式保留安全 path prefix；
- redirect 禁用；
-认证 header 与普通 header 使用不同类型；
- streaming 只能在首个下游 body 输出前 retry/fallback；
- 已开始的 stream 不与第二次尝试拼接；
- continuation state 关闭跨 deployment fallback。

## 8. 能力发现

编译期“发现”指 `compiled_definition()` 明确枚举已编译 Provider/Model，不需要网络。

远程 discovery/probe 是另一条显式管理员路径：

- 使用注册表中的固定 deployment；
- 不接受 URL、model、header 或 credential CLI 覆盖；
- 报告 `supported`、`unsupported` 或 `unknown` 观察；
- 不写回注册表；
- 不因一次失败关闭能力；
- 不把 `/v1/models` 可见性外推为功能能力。

## 9. 第二 Provider 验证门槛

在宣布抽象收敛前，至少增加一种非 OpenAI wire family，并验证：

- request 字段和 reasoning level 转换；
-认证/header；
-模型发现；
-错误与 retry hint；
-非流式响应；
-流式 terminal；
-function tool identity；
-未知字段和不支持能力的 fail-closed 行为。

## 关联文档

- [代码注册表与路由](configuration-and-routing.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [能力探测](../implementation-status/capability-probing.md)
