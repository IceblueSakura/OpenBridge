# 00：范围、术语与不变量

## 1. 产品定位

OpenBridge 保持为单个配置所有者管理、单进程、loopback、headless 的个人网关。功能宽度可以扩展，但部署复杂度不向企业控制面演进。

目标能力：

- OpenAI-compatible Chat Completions、Responses、Embeddings；
- 未来按明确需求增加 Images、Audio、Files 或其他 operation；
- 多 Provider、同名 Public Model、多 source fallback；
- image/audio/file 与未来 video 的同协议 Native 保真；
- operation-specific Models contract、zero-egress preflight、受控 retry/cancel 和有界观测。

明确不做：

- 多租户控制面、在线用户管理、计费、动态配额和 GUI；
- 请求指定 Provider、endpoint、credential、Route 或 header；
- Provider `/models` 动态注册、运行时 capability negotiation 或插件 DSL；
- 按请求能力、价格、健康或延迟重排 candidate；
- 万能媒体转换、下载代理、OCR、转码或自动工具执行。

## 2. 五个正交轴

| 轴 | 示例 | 事实 owner |
|---|---|---|
| Operation | Chat Completions、Responses、Embeddings Create、未来 Images/Audio | `core/operation` 与 operation pipeline |
| Canonical task | Generation、Embedding、SpeechRecognition、SpeechSynthesis、VoiceDesign、VoiceClone | `models/` |
| Modality | text、image、audio、file、video | task payload 中的模型事实 |
| Executable capability | source、format、detail、limits、streaming、tools、state | Provider ceiling 与 Upstream API profile |
| Resource affinity | response ID、file ID、voice/resource owner | Registry execution contract 与未来 resource 域 |

不得用一个轴替代另一个：模型有 image modality 不会自动打开 endpoint image source；Chat wire 承载 ASR 不等于存在标准 Audio operation；`file_id` 也不是普通 inline file source。

## 3. 必须保留的不变量

1. Model、Provider、Target、Upstream API、Route 和 Public Model 只由受信 Rust 注册，启动后形成 immutable registry。
2. Canonical facts → Provider ceiling → Upstream API narrowing → Route contribution → Public operation interface 只能单向收窄。
3. 每个 Public Model operation 的 capability contract 与固定 candidate 列表由同一次编译产生。
4. Analyzer 只提取 wire facts；preflight 只读取固定 interface；planner 才展开 candidate。
5. Preflight 通过后不按请求能力筛选、跳过或重排 candidate。
6. Bridge 只转换明确证明的 Generation 共同语义；媒体、资源 identity 和 Provider 私有状态默认不进入 Bridge。
7. Adapter 只能产生受信相对 URI；Target 才绑定 origin、credential 和 fault domain。
8. Standard Models 不泄露拓扑；扩展 Models 只投影 downstream-safe facts。
9. 请求正文、Base64、URL query、filename、resource ID、credential 和私有输出不得进入普通指标标签或错误上下文。

## 4. 允许破坏的范围

可以直接替换：

- 内部 Rust 类型、module path、registry definition 和 compiler API；
- Provider registration、test builders 与 synthetic fixtures；
- `/openbridge/v1/models` 扩展 schema；
- 未发布原型字段与内部 naming。

默认保持：

- 已支持 `/v1/*` 的标准 OpenAI wire、错误和 JSON/SSE terminal；
- Public Model ID，除非其 task/operation 语义本身错误；
- credential、trusted egress、loopback 和日志安全边界。

不得保留 legacy alias、双读写、双 runtime path 或无意义 schema shim。每次 direct replacement 必须原子更新源码、OpenAPI、fixtures、测试和文档。

## 5. 明确拒绝的抽象

- `HashMap<String, Value>` capability、字符串 feature flag、`dyn Capability` 或 runtime plugin；
- 一个万能 `Media`/`ContentPart` AST 统一 image/audio/file/video；
- 一个万能 analyzer、renderer、bridge 或 operation trait 承担所有 wire 语义；
- 跨 Provider 共享 capability fact constant；
- 把 Public DTO 当执行配置，或从 DTO 反向驱动 preflight。
