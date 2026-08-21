# 07：阶段 5B——首个真实新 operation 与收口

## 状态

阶段 5A typed file input 已完成；当前实现见[Native 文件输入](../../implementation-status/features/native-file-input.md)。本页只保留尚未实施的 5B、legacy 清理与后续扩展规则。

## 5B：首个真实新 operation

必须基于已批准的客户端需求、当前官方 wire、一个明确 Provider/Target profile 和独立验收边界选择。候选可以是标准 Images 或 Audio
operation，但不能是 Files lifecycle、异步 resource job 或 Realtime session。

新 operation 必须独立定义：

- endpoint、method、content type 与 strict request catalog；
- canonical task binding 与 Provider/Target executable profile；
- request/response budget、JSON/SSE/binary/multipart terminal；
- retry eligibility、replay、commit、cancel 与 observability units；
- private registry interface、Public Models 投影及 Bridge/resource 明确允许或拒绝。

只有现有 Models v1 容器无法准确表达该客户端合同时，才为 Models v2 建立独立 current focus；新增 operation 本身不自动触发 v2。

## 共同纵向路径

```text
OpenAPI/router/auth/body limit
→ operation analyzer
→ typed request facts
→ Public Model preflight
→ fixed RoutePlan
→ execution coordinator
→ Provider operation adapter
→ loopback upstream
→ response driver/terminal
→ commit/cancel/observability
```

不能只测试 constructor、adapter body 或 mock compiler。

## Legacy 清理

新 operation 纵向证明完成后，删除剩余旧 capability/module/type、compatibility conversion、operation-only API key、固定 private operation fields、
registration media mutation、duplicate builders、orphan fixtures 和 stale links。不得以 TODO、feature flag 或 dead branch 保留旧路径。

## 退出门

- typed file 与新 operation 的 focused tests 全绿；
- full Rust、corpus（若改动）、OpenAPI、link 和 `git diff --check` 全绿；
- requirements 与 implementation status 只记录实际合同、事实和命令；
- `current-focus.md` 恢复为空。

## 后续扩展

每个后续 operation 仍使用单独 current focus：wire/limits/errors → RED fixture → operation/profile → registry interface → pipeline/adapter →
Provider evidence。框架存在不授权批量打开未验证 capability。