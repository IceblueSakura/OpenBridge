# M03 Provider Adapter

## 模块职责

Provider Family 以编译期代码定义：

- 支持的 wire protocol 和 endpoint profile；
- credential 类型与认证 header；
- 请求相对路径和必要 header；
- 响应/SSE 终态；
- Provider 错误和 retry 分类；
- capability 上界。

Deployment 只提供受信运行时数据，不注入任意认证逻辑、转换脚本或出站行为。

## 当前状态

- 已实现 `OpenAi` Provider Family；
- 支持 API key；
- 支持 Chat Completions 与 Responses 原生路径；
- 已有 request/header/auth/response/error/capability contract tests；
- 尚未实现第二 Provider Family；
- 尚未形成可复用于多个 Family 的完整 conformance suite。

## Provider conformance

每个 Family 至少验证：

- request path、method、model 和未知字段；
- safe/sensitive header 分离；
- credential binding；
- JSON/SSE response；
- terminal、error、EOF 和 cancel；
- capability 上界；
- rate-limit/temporary failure/ambiguous failure 分类；
- safe error fields、Provider request id、`Retry-After` 和 rate-limit header allowlist；
- timeout、retry、cooldown 和 fallback 边界。

## 详细资料

- [Rust Provider adapter 与数据流](../architecture/rust-provider-adapter-dataflow.md)
- [Provider 韧性需求](../requirements/provider-resilience.md)
- [当前实现](../implementation/current-implementation.md)
- [参考项目比较矩阵](../research/project-comparison-matrix.md)
