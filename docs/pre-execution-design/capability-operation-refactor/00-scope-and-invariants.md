# 00：范围、术语与不变量

## 1. 产品范围

OpenBridge 保持为单个配置所有者管理、单进程、loopback、headless 的个人网关。本轮只扩展 model-bound inference operation：

- 当前 Chat Completions、Responses、Embeddings；
- 未来有明确 wire、Provider 和验收需求的 Images、Audio 或其他模型调用；
- image/audio/file/video 等输入输出 modality 及其 Native typed profile；
- 多 Provider、同名 Public Model、固定多 source fallback。

Files/Uploads、Vector Stores、异步 Video job、Realtime session、response/resource lifecycle 属于未来独立 resource/session domain，
不经过本轮 Public Model operation 架构。file content part 只是媒体输入，不等同于 Files API。

## 2. 正交事实轴

| 轴 | 回答的问题 | 示例 |
|---|---|---|
| Operation | 客户端通过哪个 API 动作调用 | Chat Completions、Responses、Embeddings Create |
| Canonical task | 模型执行什么语义任务 | Generation、Embedding、SpeechRecognition、SpeechSynthesis |
| Modality | 输入输出是什么数据 | text、image、audio、file、video |
| Executable capability | 该 API 路径具体允许什么 | source、format、limits、streaming、tools、state |
| Resource affinity | opaque identity 由谁签发并绑定到哪里 | response ID、file ID、voice/resource owner |

一个 operation 可承载不同 task，同一 task 也可由多个 operation 暴露。modality 不能推导 task 或 endpoint；resource ID 也不是普通 inline/remote source。

## 3. 固定不变量

1. Model profile、Provider、Target、Upstream API、Route 和 Public Model 只由受信 Rust 注册，启动后形成 immutable registry。
2. 每个 canonical executable profile 只有一个 task；task-specific facts 由该闭合 variant 独占。
3. Upstream API 使用 typed `(operation, task)` key；task 必须匹配引用的 canonical profile。
4. 每个 Public Model operation interface 显式绑定一个 task；其全部 candidates 必须 task-compatible。
5. Task 在启动编译时选定，业务请求不得按 body shape 选择或切换 task。
6. Canonical facts → Provider ceiling → Upstream API narrowing → Route contribution → Public interface 只能单向收窄。
7. 每个 operation 的 capability contract 与固定 candidate 列表由同一次编译产生。
8. Analyzer 只提取 wire facts；preflight 只读取固定 interface；planner 才展开 candidate。
9. Preflight 通过后不按请求能力筛选、跳过或重排 candidate。
10. Bridge 只转换已证明的 Generation 共同语义；媒体、resource identity 和 Provider 私有状态默认不进入 Bridge。
11. Adapter 只能产生受信相对 URI；Target 才绑定 origin、credential 和 fault domain。
12. Standard/extended Models 都不泄露执行拓扑；Public DTO 不得反向驱动 private preflight。
13. 正文、Base64、URL query、filename、resource ID、credential 和私有输出不得进入普通指标标签或错误上下文。

## 4. 变更边界

获准切片可以直接替换内部类型、module path、registry/compiler API、Provider registration、builders 和 fixtures。默认保持：

- 已支持 `/v1/*` 的标准 wire、错误和 JSON/SSE terminal；
- 当前唯一的扩展 Models schema v1，直到真实 schema 需求触发单独替换；
- Public Model ID，除非其 operation/task 语义本身错误；
- credential、trusted egress、loopback 和日志安全边界。

禁止 legacy alias、双读写、双 runtime path 和无意义 schema shim。行为或公共 schema 真正改变时，必须原子更新源码、OpenAPI、fixtures、测试和文档。

## 5. 明确拒绝的抽象

- `HashMap<String, Value>` capability、字符串 feature flag、`dyn Capability` 或 runtime plugin；
- 一个万能 `Media`/`ContentPart` AST 统一 image/audio/file/video；
- 一个万能 analyzer、renderer、bridge 或 operation trait 承担所有 wire 语义；
- 跨 Provider 共享 capability fact constant；
- 在没有真实跨 task 模型前引入运行时 task set 或 request-shape task routing。
