# Protocol gateway 与 semantic model 调研索引

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | Bifrost `dev` @ `7e26cffbd47cd295f35b64176bfbb721fdd0924a`；LiteLLM `litellm_internal_staging` @ `5e4b3838aabf00d135be800404d03728c8afa506`；TensorZero `main` @ `62eb8f63e8ec62018d70420dbf1a8c5d1c026315`；Vercel AI SDK `main` @ `69428b1f8b037e4d118fb4853428d5c4e620493c`；Portkey Gateway `main` @ `669825cbe89ee51569918b8f78a9db486fd69dd4`；Helicone AI Gateway `main` @ `9649b27bdc9fb0907d359e899894102a15f3a085` |
| Last reverified | 2026-08-30，本地只读源码与测试源码复核 |
| Scope | 多协议 schema/conversion、provider adapter、tool/reasoning/state、streaming、routing/runtime 与可吸收测试资产 |
| Evidence boundary | 静态源码；未构建或启动这些项目，未调用真实 Provider，未证明其生产行为、性能、Provider 当前兼容性或 OpenBridge 设计选择 |
| Recheck trigger | 任一项目采用、升级、复制测试/fixture，或其 schema、adapter、streaming、routing、license 发生变化时 |

## 项目级前置

| 优先级 | 项目 | 许可证 | 调研入口 |
|---|---|---|---|
| P0 | Bifrost | Apache-2.0 | [多协议 schema、Provider 转换与 streaming](bifrost.md) |
| P0 | LiteLLM | MIT；`enterprise/` 另有条款 | [既有 LiteLLM 索引](../litellm/README.md) |
| P0 | TensorZero | Apache-2.0 | [Provider-native capability 与 semantic types](tensorzero.md) |
| P1 | Vercel AI SDK | Apache-2.0 | [Provider-neutral language model types](vercel-ai-sdk.md) |
| P1 | Portkey Gateway | MIT | [Provider adapter 与 middleware](portkey.md) |
| P1 | Helicone AI Gateway | GPL-3.0 | [Rust runtime gateway、routing 与 observability](helicone.md) |
| P2 | OpenRouter | 闭源服务；只引用官方公开资料 | [OpenRouter API 调研](../providers/openrouter-api.md) |

许可证以各固定 checkout 根目录的 `LICENSE` 为准；本表不是法律意见。外部测试默认只借鉴独立场景并自主编写 synthetic fixture。复制代码、payload 或 fixture 前必须重新核对具体文件的 license、来源和 attribution。

## 统一调研维度

每个叶文档区分：

1. ingress、canonical/protocol types、Provider adapter、routing、transport、streaming、error 与 observability；
2. decode/encode 是 canonical semantic model、protocol-shaped intermediate，还是 pairwise conversion；
3. function、hosted/provider-native tool、reasoning、opaque state、usage 与 terminal 的表达；
4. unsupported、normalization、warning、silent drop 与 fail-closed 边界；
5. 可独立重写为 deterministic fixture 的测试场景；
6. `Adopt`、`Adapt`、`Avoid` 和 `Open Questions`，其中“采用”只表示研究价值，不构成本地实施承诺。

跨项目共性只进入 [cross-project](../cross-project/README.md)，并链接全部项目级前置。OpenBridge 当前源码事实、产品合同和实施焦点不写入本目录。
