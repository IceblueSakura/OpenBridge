# Codec 与扩展验收基线

来源：已整合的[历史测试研究](semantic-baseline.md#8-历史来源追溯)、[Responses 标准快照](responses-standard.md)和[上游同步](upstream-sync.md)。综合日期 2026-09-23。外部项目测试与模型 benchmark 不自动成为本地 oracle；引入资产或升级 SDK/profile 时重核版本、许可与独立预期。

## 1. 准入表必须按语义分支

每个已实现标准字段、item、event 或扩展要回答：

| 问题 | 验收内容 |
|---|---|
| 来源 | 官方 schema/guide、固定 SDK 或具体 extension profile；不能仅引用本地 encoder |
| Owner | task、request/response context、delivery、fidelity 或 scoped extension；只有一个值权威 |
| Presence | required/optional、absent/null/empty/default 的等价或区别；依赖其他字段的合法组合 |
| 双向映射 | 独立 wire→IR 和 IR→wire 预期；必要的 profile 差异 |
| Transform | 插入/替换/删除/重排；requirements 更新；dependent metadata 的保留或失效 |
| State | item/call/part/response、continuation、scope、finality 与 terminal |
| 拒绝 | unknown、错类型、身份/状态冲突、不可表示目标、跨 issuer 或错误后恢复 |
| 资源 | bytes、items、深度/nodes、schema references、padding、partial payload 与总状态预算 |

标准全景不等于每轮实现全部，但每轮完成的范围必须真正闭合。媒体和状态服务缺实现不是删掉标准目标的理由。

## 2. 四个独立验证层

1. **语义与 codec**：typed invariant、正反映射、删除不复活、扩展 scope；无需网络。
2. **字节与生命周期**：严格 JSON、SSE framing、UTF-8/CRLF、分片独立预算、EOF、背压、cancel、pre/post-commit。
3. **固定消费者**：SDK create/stream/parse 及 structured replay、Agent tool loop；版本固定，synthetic loopback。
4. **外部实际执行**：固定 Provider/account/model/payload 的能力和错误；需要单独授权，不能由前三层替代。

模型输出质量和长期负载再单独评价，不要求 codec 引入大型真实会话或 benchmark 数据。Open Responses compliance 也必须与 OpenAI 标准版本分别标记。

## 3. 必须保护的闭合反例

- 已验证 IR 的外层 presence 与有效子字段不能矛盾，更不能在 encode 时静默省略约束。
- low-level input/snapshot 简写不能放宽完整 response 的必填 id/status；显式合成与上游缺字段分别处理。
- event 缺 required payload、跨 kind 字段、非法 status 不得被忽略后继续成功。
- JSON 重复键必须在失去原始键序列前按严格边界处理。
- Schema 不能只检查 object/bytes；嵌套类型、strict/profile、引用与顺序分别验证。
- 每种 SDK derived view 都需要独立的回放准入与一致性规则；一种派生 view 通过不证明另一种也支持。
- 标准 phase、configuration update、媒体、工具与 state 必须分支验收，不靠一个两轮 fixture 声明完整。

OpenBridge 当前具体违反项、复现与源码证据统一在[迁移缺口](../architecture-v2/migration.md#当前已知闭合缺口)维护，本页只定义方法与来源边界。

## 4. 属性顺序、JSON 与 Schema

JSON object 通常无语义顺序，但 Structured Outputs 官方明确承诺按 schema key 顺序生成输出。实现应明确 schema owner 的顺序表示，不能用普通 map 的排序规范化覆盖该合同；是否启用保序解析必须以实际实现验证。

重复 JSON key 在进入 Value 前解决；资源验证不能在已经无界 parse/clone 后才发生。递归 schema 的 `$ref` 是引用图，不等于无界输入 nesting；两种预算需要分开，不能为阻止栈爆而拒绝所有合法递归 schema。

## 5. SDK 升级门槛

当前消费者与传递依赖在 `tests/sdk/pyproject.toml` 和 `uv.lock` 固定；版本与执行入口见[开发指南](../development.md)。升级应比较 required/null/default、derived views、items/events 和输出 schema，再更新锁文件并执行严格两轮 JSON/SSE gate；其通过不消除其他未覆盖分支的验收缺口。

SDK 宽松解析成功不证明完整 wire 正确；严格模型验证也不证明 gateway state machine。新版本 type union 可包含尚无完整 endpoint 参数/资源语义闭环的分支，需要记录冲突而非补猜。

## 6. 接受扩展的专项反例

- 合法 namespace + 错误 attachment/schema/version 拒绝；
- final token 与 owner、origin、principal scope 不匹配拒绝；
- 同 turn 重放允许，跨 turn/auth owner 不沿用 Codex sticky state；
- 扩展不得覆盖标准字段或传入上游地址/认证；
- 稳定 local identity 不因排序变化被重建；删除 owner 后不回填旧 signature/annotation；
- 标准化后的字段不再双写标准 owner 和 extension owner；
- 不可迁移的 media/resource 引用不在 fallback 中自动跨目标转移。

## 7. 资产与证据卫生

优先自主编写小型 synthetic fixture，并保留源测试路径、commit、许可与抽象行为。GPL/AGPL、限制商业使用的数据、enterprise 目录或混合来源 benchmark 不能未经审查复制。原始源码/Provider transcript 不是默认测试数据；不保存密钥、账户、真实会话、媒体或签名 URL。
