# 当前开发焦点

## 状态

**唯一活动焦点：v1 deployment model constraints。**

此焦点以当前 `main` 的实际实现为基线：`[[models]]` 已提供 `id`、名称、描述、
`context_length.input/output`、`supported_parameters` 与 reasoning 状态；deployment 目前仅引用
Model，尚不能声明某个订阅/账号/endpoint 的更小有效能力。目录同步、`context_length.total`、
`local_correction`、新 Provider Family 和管理 API 都不属于本焦点。

## 行为

同一 logical Model 被多个 deployment 引用时，每个 deployment 可以配置只会收窄的
`model_constraints`。例如 Codex 订阅 deployment 的 output 上限或 reasoning 可用性更低，不得
影响同一模型的 API deployment；路由只选择满足请求与该 candidate 有效限制的 deployment。

## 对应功能需求

- [配置、凭证与受信运行边界](../functional-requirements/configuration-and-credentials.md)：CFG-04、CFG-05。
- [Provider 韧性需求](../functional-requirements/provider-resilience.md)：模型/deployment/协议组合的保守能力筛选。

## 先失败的测试

1. `deployment_constraints_only_reduce_effective_model_capabilities`
   - 两个 deployment 引用同一个 Model；其中一个将 output 上限和 reasoning 收窄。
   - 当前 schema 因未知 `model_constraints` 拒绝配置，因此测试应先失败。
2. `deployment_constraints_do_not_widen_model_metadata`
   - 尝试把模型的未知/不支持 reasoning 或未声明参数提升为可用，必须在配置加载阶段失败。
3. `deployment_constraints_select_the_unconstrained_candidate`
   - 当第一个 candidate 被其 constraint 排除、第二个同模型 deployment 兼容时，路由应选择第二个。
4. `deployment_constraints_reload_is_atomic`
   - 新 constraint 无效时，旧 snapshot 和 in-flight request 不变；有效 reload 仅影响后续请求。

## 最小实现边界

- 在 route schema v1 的 deployment 文档与编译快照中加入 `model_constraints`。
- 仅支持现有模型字段的收窄：`context_length.input/output`、reasoning 和
  `disabled_parameters`；数值限制取已知值的最小值，参数只能删除。
- 让 pipeline 使用 candidate 的有效 output/reasoning 元信息；`input` 继续只保存为 metadata，
  不引入 tokenizer 或 JSON 字节数预检。
- 为加载、路由、reload 和示例配置补确定性测试与最小文档。

明确不做：OpenRouter 同步/cache、`catalog promote`、`context_length.total`、全局
`local_correction`、自动 capability 探测写回、新 Provider、真实 Codex/OpenRouter 调用、管理 API。

## 本次验证

- 本地：`cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`。
- 证据：仅 loopback fixture 与配置/路由契约；不需要真实 Provider 凭证。
- 未完成前，不将 deployment constraints 写入[当前实现说明](../implementation-status/current-implementation.md)。

## 完成门禁与计划清理

完成后必须按顺序执行：

1. 只把测试证明的 constraint 行为、边界和验证命令写入
   [当前实现说明](../implementation-status/current-implementation.md)。
2. 从[配置与路由](configuration-and-routing.md)的短队列中删除本阶段及其已过时的设计细节；
   不保留“已计划”叙事。
3. 将本文重置为“暂无活动焦点”。
4. 重新检查 live source、工作区状态、测试和实施现状后，才选择下一项最小行为；不得依据本文件
   自动进入后续阶段。

## 关联文档

- [实施计划索引与生命周期](README.md)
- [配置与路由](configuration-and-routing.md)
- [当前实现说明](../implementation-status/current-implementation.md)
