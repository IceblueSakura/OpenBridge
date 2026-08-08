# 当前测试资产树

## 状态

**Confirmed。** 当前 checkout 共有 335 个可执行测试：290 个 Rust 默认测试和 45 个 Python testkit 测试。Rust
测试当前没有 ignored test；51 个 canonical wire case 与 9 个 semantic case 是测试输入与判定 oracle，不计入可执行测试总数。

本文按功能所有权维护测试树。叶节点覆盖当前每一个可执行测试所在的 target、文件或互斥命名模块；括号内是该叶节点实际收集的
test case 数量。一个物理 target 跨越多个功能时，使用不重叠的 Rust 模块前缀拆分，例如
`tests/forwarding_contract.rs` 的 `admission::*`、`chatgpt::*`、`models::*`、`native::*`、`mimo::*` 和
`resilience::*`，每个测试只计算一次。

## 可执行测试树

```text
OpenBridge 可执行测试（335）
├─ Rust 默认测试（290；ignored 0）
│  ├─ HTTP ingress 与下游认证（19）
│  │  ├─ src/lib.rs :: ingress::*（8）
│  │  ├─ tests/downstream_auth_contract.rs（2）
│  │  ├─ tests/ingress_contract.rs（6）
│  │  └─ tests/forwarding_contract.rs :: admission::*（3）
│  ├─ 启动配置、用户与受信凭证（38）
│  │  ├─ src/lib.rs :: credential::store::*（3）
│  │  ├─ tests/config_contract.rs（17）
│  │  ├─ tests/credential_store_contract.rs（1）
│  │  ├─ tests/startup_contract.rs（3）
│  │  └─ tests/upstream_credential_config.rs（14）
│  ├─ ChatGPT OAuth2 生命周期与数据面（20）
│  │  ├─ src/lib.rs :: oauth2_credentials::*（11）
│  │  ├─ src/bin/openbridge-auth.rs（1）
│  │  ├─ tests/oauth2_login_cli.rs（2）
│  │  └─ tests/forwarding_contract.rs :: chatgpt::*（6）
│  ├─ Registry、Models、能力预检与路由（42）
│  │  ├─ src/lib.rs :: providers::catalog::route_compiler::*（1）
│  │  ├─ src/lib.rs :: registry::availability::*（2）
│  │  ├─ tests/capability_definition_contract.rs（6）
│  │  ├─ tests/example_config.rs（13）
│  │  ├─ tests/forwarding_contract.rs :: models::*（3）
│  │  └─ tests/native_routing_contract.rs（17）
│  ├─ Provider adapter、probe 与上游 transport（47）
│  │  ├─ src/lib.rs :: probe::*（10）
│  │  ├─ src/bin/openbridge-probe.rs（2）
│  │  ├─ src/lib.rs :: provider::*（5）
│  │  ├─ src/lib.rs :: providers::openai_compatible::*（2）
│  │  ├─ src/lib.rs :: transport::upstream::*（7）
│  │  ├─ tests/provider_boundary_contract.rs（14）
│  │  └─ tests/provider_contract.rs（7）
│  ├─ Native generation、图片输入与 SSE 解码（14）
│  │  ├─ tests/forwarding_contract.rs :: native::*（5）
│  │  ├─ tests/forwarding_contract.rs :: mimo::*（3）
│  │  └─ tests/sse_contract.rs（6）
│  ├─ Retry、fallback、credential health 与取消（32）
│  │  ├─ tests/forwarding_contract.rs :: resilience::*（24）
│  │  └─ tests/process_replay_contract.rs（8）
│  ├─ Chat ↔ Responses Protocol Bridge（29）
│  │  ├─ tests/bridge_conversion_contract.rs（12）
│  │  ├─ tests/bridge_forwarding_contract.rs（9）
│  │  └─ tests/protocol_bridge_replay.rs（8）
│  ├─ OpenAI-compatible Embeddings（27）
│  │  ├─ tests/embedding_definition_contract.rs（5）
│  │  ├─ tests/embedding_registry_contract.rs（2）
│  │  └─ tests/embedding_forwarding_contract.rs（20）
│  └─ Observability、metrics 与 traces（22）
│     ├─ src/lib.rs :: observability::*（5）
│     ├─ tests/observability_contract.rs（13）
│     ├─ tests/otlp_metrics_contract.rs（2）
│     └─ tests/otlp_trace_contract.rs（2）
└─ Python protocol corpus/testkit（45）
   ├─ tools/corpus/tests/test_corpus.py（14）
   ├─ tools/corpus/tests/test_semantic.py（8）
   ├─ tools/corpus/tests/test_sse.py（3）
   ├─ tools/corpus/tests/test_testkit.py（11）
   └─ tools/corpus/tests/test_verifier.py（9）
```

Rust 树包含 24 个 integration test target、`src/lib.rs` 和两个有测试的 binary target。`src/main.rs` 与 Rust
doc-tests 当前各收集 0 个测试，因此不作为可执行叶节点；当前也没有 benchmark。

## Canonical oracle 树

`testdata/cases/` 中的 51 个 case 是 Rust replay 与 Python testkit 可读取的共享 wire oracle，不是 51 个额外的可执行测试：

```text
testdata/cases（51 个 canonical wire case；非可执行）
├─ bridge（19）
│  ├─ chat_to_responses（6）
│  └─ responses_to_chat（13）
├─ faults（10）
│  ├─ chat_native（1）
│  └─ responses_native（9）
├─ native（20）
│  ├─ chat_native（9）
│  └─ responses_native（11）
└─ transport（2）
   ├─ chat_native（1）
   └─ responses_native（1）
```

`testdata/semantic-cases/` 另有 9 个协议无关 function-tool case，覆盖 no-tool、单调用参数、歧义选择、缺参澄清、
none/required/forced tool choice、无序并行 calls 和结果事实；每个 case 的 reference trace 是正向 oracle，不是额外可执行测试。

seed `20260726` 生成的 342 个 SSE wire variants 是从 canonical wire case 确定性派生的 byte-fragmentation 输入；它们属于
Python SSE parser 的参数空间，不是 342 个独立测试，也不是应提交的 canonical 文件。

## 所有权与证据边界

- Rust 测试负责 OpenBridge runtime 行为：ingress、registry/routing、Provider contract、Native/Bridge 转发、retry/fallback、
  cancellation、OAuth2、Embeddings 与 observability。
- Python 测试负责 corpus 与独立 testkit：schema/provenance/secret lint、确定性 generation/report/pack、SSE fragmentation、
  Mock Server/Client loopback、observation verifier 和 normalized function-tool semantic verifier。
- Canonical corpus 是两层共享的只读协议输入。fixture 存在只证明 oracle 已登记；Rust replay 或 Python loopback 也只证明其直接执行的
  边界。
- 当前默认测试树不包含外部 OpenAI SDK、Codex、Hermes、真实 Provider、负载或长期运行测试；这些验收层不能由 335 个确定性测试
  代替。

## 盘点与验证证据

2026-08-08 在当前 Windows checkout 执行：

```powershell
cargo test --locked
uv run --project tools/corpus pytest tools/corpus/tests
```

结果：Cargo 执行并通过 290 个测试；pytest 收集并通过 45 个测试。只读解析 `testdata/cases/**/case.json` 得到 51 个唯一 wire case manifest，
目录分布为 19/10/20/2；`testdata/semantic-cases/**/case.json` 得到 9 个唯一 semantic case。Rust baseline 与 Python test body
的实际执行结果见[协议测试语料与工具](protocol-test-corpus.md)；没有执行真实 Provider、外部 SDK、负载或长期运行验收。

## 维护规则

1. 增删、移动或重命名测试时，同步更新对应叶节点和所有祖先计数；Rust 总数必须等于所有 Rust 功能分支之和。
2. 跨功能 test target 必须按互斥模块前缀拆分，不能把同一测试重复登记到多个功能点。
3. 函数级名称以 `cargo test --locked -- --list` 和 pytest `--collect-only` 的实时输出为准，不在状态文档复制 335 个易漂移的函数名。
4. `testdata/` 或 `tools/corpus/` 的契约、case 或生成规则变化时，同时更新[协议测试语料与工具](protocol-test-corpus.md)中的版本、
   验证证据和未覆盖范围。

## 相关文档

- [实施现状目录](README.md)
- [协议测试语料与工具](protocol-test-corpus.md)
- [测试补全计划](../implementation-plans/test-coverage-completion-plan.md)
- [Corpus 指南](../../testdata/README.md)
- [Testkit 指南](../../tools/corpus/README.md)
