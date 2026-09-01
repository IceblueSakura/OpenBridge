## 1. 功能验收要求

| ID     | 行为                                                                                                                                                           |
|--------|----------------------------------------------------------------------------------------------------------------------------------------------------------------|
| CFG-01 | 仓库不存在 Provider/Model route 配置文件或动态 Provider schema。                                                                                               |
| CFG-02 | 代码注册表中的重复 ID、未知引用、能力扩大、无效 reasoning/level 映射和不安全 URL 在监听前失败。                                                                |
| CFG-03 | 业务请求无法覆盖 endpoint、真实 model、credential、敏感 header 或 candidate 顺序；普通 header 只能由受信 Provider 代码声明或转换，固定 UA/header 在 hook 后应用，业务请求不能选择规则或覆盖固定值。 |
| CFG-04 | secret 不进入代码注册项、`RuntimeRegistry`、日志、错误或 probe report。                                                                                        |
| CFG-05 | 每个 Provider family 由独立、闭合的 definition owner 管理，并经单一显式 composition root 注册；不存在自动注册。                                             |
| CFG-06 | bootstrap 只控制 listener、文件位置、资源上限、HTTP client、本地 HTTP 内容日志与 telemetry 导出等进程资源策略，不能注册或修改 Provider；collector host 可由配置所有者选择。 |
| CFG-07 | listener 只允许 loopback；非 loopback 地址必须在监听前拒绝。                                                                                                   |
| CFG-08 | 用户文件中的无效 schema、重复 ID/Key、短 Key 或无启用用户会阻止启动。                                                                                          |
| CFG-09 | 下游与上游 API-key secret 只进入启动时不可变 `CredentialStore`；OAuth bundle 只进入固定 wiring 的 manager，并仅按专有 lifecycle 发布受控 token snapshot/generation。 |
| CFG-10 | 私有 upstream credential TOML 出现未知或重复 pool、空白/重复 secret 或不能解析时，会在 listener 绑定前阻止服务启动；缺失或为空的已注册 pool 会让其引用 Target 在本次启动中不可执行。 |
| CFG-11 | 同 Provider 的 Target 可引用共享 API-key pool；激活 pool 必须满足 Provider/kind 与 member 约束，未激活 pool 不要求 secret。                                               |
| CFG-12 | 多 member pool 不得用于启用 `TargetBoundContinuation` 的 Responses API；普通 Target-bound、无 continuation 的 API 不因此失去 credential rotation。                         |
| CFG-13 | 五个 ChatGPT Responses-native Target 共用独立 OAuth pool；四个分别属于 ChatGPT-only Public Model，Sol Target 属于还含 OpenAI source 的 `gpt-5.6-sol`；请求和 probe 不接受本机 Codex auth/environment/terminal/executable selector。 |
| CFG-14 | Provider 实例唯一拥有一个受信 BaseURL；Target 必须引用已注册实例，不同 URL/区域使用不同实例，业务请求不能覆盖实例或 URL。                                            |
| CFG-15 | 每个 Target 对每个 `OperationKind` 最多注册一个 Upstream API；Route、probe、telemetry 与 continuation issuer 使用 typed upstream operation，不依赖 API 字符串 ID。 |
| CFG-16 | Upstream API 的 operation 只由 capabilities variant 决定；当前 transport 由 operation 固定，注册表不保留独立 operation、transport 或无执行语义的 endpoint profile。 |
| CFG-17 | 主服务在配置验证后、listener 前输出配置态 Provider/Public Model 可用/不可用双表；分类复用 active Target/执行接口且不触发 Provider egress，不输出 credential 或内部拓扑，也不把配置态结果声明为真实健康。 |
| CFG-18 | 随附开发配置的四个本地下游 HTTP 内容日志开关显式全开，自定义配置缺表/缺字段时回退关闭且可独立覆盖；未知 logging 字段阻止启动，敏感 header 始终脱敏，body capture 有界且不进入 OTLP，开关不改变请求/响应字节、路由或终态。 |
| CFG-19 | 任一通用 Generation interface 可执行时要求非空 `default_instructions`；仅有 Embeddings/专用音频 task 时不要求。客户端有效值优先，默认值在 candidate 展开前统一解析，Provider、probe 或请求不能另行覆盖。 |
