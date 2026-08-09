# 当前开发焦点

## 状态

**已获准实施：严格的 Chat/Responses 顶层参数分类与候选级 egress 处置。**

本焦点只处理 generation request 的标准顶层参数。目标是让 Native 与 Bridge 在上游调用前采用同一套确定语义，使未知、
不支持和无法跨协议表达的问题尽早暴露，而不是依赖上游报错或在转换中静默丢失。

## 可观察行为

每个下游 Chat Completions 或 Responses 请求的顶层字段必须先按源协议类型化目录分类，再按固定 Public Model 契约和实际
Route candidate 得到以下唯一处置之一：

- 已知、接口接受且 candidate 能处理：Native 保持标准 wire 语义，Bridge 通过显式映射转发；
- 已知、属于闭合生成提示集合且所选 Upstream API 有明确忽略规则：OpenBridge 接受请求，并在该 candidate 进入第一个无法
  表达它的 Bridge/Provider 转换前静默删除；
- 已知但固定接口不支持，或目标虽然支持但 Bridge 无法完整表达：在任何上游调用前返回
  `unsupported_model_capability`；
- 不在源协议类型化目录中的未知顶层字段：Native 与 Bridge 都在任何上游调用前返回稳定的
  `unknown_parameter`；
- 已知但类型、枚举或结构非法：返回稳定的 `invalid_request_error`，不得伪装成未知或不支持。

请求只执行一次固定接口预检，不得按参数筛选或重排 Route。每个 candidate 必须从原始下游 body 独立构造；前一个 candidate
删除过的字段不得影响 fallback candidate，因此一个 API 忽略、另一个 API 支持的参数在 fallback 时仍按后者规则转发。
`NativeFirst` 与 `SourceFirst` 只影响既有 candidate 顺序，不改变参数分类和处置。

## 需求边界

- 同步收敛[网关 API 与客户端兼容](../functional-requirements/gateway-api-compatibility.md)中的 Native wire-preservation 与普通参数
  兼容规则：未知下游请求字段不再属于 Native 透明透传承诺；Native 上游响应中的未知合法 JSON 字段/SSE event 不在本焦点内，
  继续保持现有响应边界。
- `supported_parameters` 继续表示 OpenBridge 接受该接口的顶层参数。一个 Route 原样转发、另一个 Route 明确忽略时仍可参与
  公共交集；任一 Route 既不能转发、也没有类型化忽略规则时不得公开该参数。
- 首批允许无条件忽略的闭合集合仅包含 `frequency_penalty`、`presence_penalty`、`temperature`、`top_p` 和 `seed`。
- `n`、`logprobs`、`top_logprobs` 与 `include_reasoning` 会改变可观察输出数量、结构或 reasoning 可见性；上游不支持时必须从
  有效接口收窄并拒绝请求，不得继续作为忽略参数。
- streaming、reasoning level/开关、tools/tool choice、structured output、state/continuation、媒体输入输出、输出 token 上限、
  认证和 Provider 私有扩展继续保持 fail closed，不能进入普通参数忽略集合。
- “已知”只由 Chat/Responses 的代码内类型化顶层字段目录决定；OpenRouter 元数据、Provider 文档或真实测试只用于证明某个
  Upstream API 的支持/忽略事实，字段缺失不能自动推导为不支持。运行时不得下载元数据或改变策略。
- 未知字段即使值为 `null` 仍是未知。已知字段的 `null`/`false` 是否表示未请求能力，只能由该字段已有的类型化语义决定，
  不增加通用绕过规则。
- 本阶段不提供任意 `extra_body` 或其他 Provider-specific passthrough。未来扩展必须单独进入功能需求并采用受信、类型化注册。

## 首个失败测试

在修改生产代码前，新增一个表驱动的 generation 参数处置契约并确认当前实现至少在以下断言上失败：

1. Native 请求携带未知顶层字段时返回 HTTP 400、`code = unknown_parameter`、精确 `param`，且 transport attempt 为零；
2. 同一未知字段走 Bridge 时得到相同的下游错误和零上游调用；
3. Kimi K3 Chat Native 与 Responses-to-Chat Bridge 携带 `temperature` 时均通过固定接口预检，实际 Kimi candidate 的上游 body
   不包含该字段；
4. Kimi K3 的 `n`、`logprobs` 或 `top_logprobs` 不再静默删除，而是作为已知但不支持参数在 egress 前拒绝；
5. 第一个 synthetic candidate 忽略参数并在首输出前失败后，支持该参数的 fallback candidate 仍收到原始字段。

同一测试切片随后补齐：已知支持参数的 Native 保真和 Bridge 显式映射、Bridge 已知但不可表达参数的拒绝、未知 JSON Schema
property 名不被误判为顶层参数、忽略/禁用集合的启动验证，以及 `supported_parameters` 对转发/忽略/拒绝三种 Route 贡献的聚合。

## 最小实现边界

1. 增加 Chat/Responses 协议级类型化顶层参数目录；公共 Models DTO 仍输出稳定 wire-name 字符串，不新增第二套重复参数列表。
2. 在 generation analysis 中记录请求实际出现的已知参数和首个未知字段，并在固定接口 preflight 中区分 unknown、invalid、
   unsupported 与 accepted。
3. 将普通参数策略保持为 Upstream API 级闭合类型：`disabled_parameters` 表示该 API 不接受并收窄接口，
   `ignored_parameters` 只允许上述五个生成提示，二者不得重叠。
4. 在 registry 编译时验证每条可执行 Route 对公开参数只能贡献“可转发”或“明确忽略”；Bridged Route 的可转发还必须经过
   当前转换方向的类型化 representability 检查。
5. 为每个 Route candidate 从原始 body 建立独立请求，在 Bridge/Provider shape 转换之前应用该 API 的忽略规则，并在最终
   transport adapter 保留“忽略字段不得出现”的确定性防线。
6. 增加稳定且脱敏的 `unknown_parameter` 错误映射；已知但不支持或 Bridge 不可表达继续使用
   `unsupported_model_capability`，两者都返回安全的字段名而不暴露 Provider、Target、Route 或 credential。
7. 更新受影响的 Kimi、ChatGPT 等 Upstream API 参数规则：只保留有证据的五类提示忽略；输出语义字段改为显式收窄。
8. 完成后同步功能需求、Models/Native/Bridge/Provider 实施现状与测试清单，并将本文件恢复为空焦点。

## 非目标

- 不改变 Public Model、Route、Provider 或 credential 的选择方式，不调整 retry、fallback、cooldown、状态亲和或路由顺序。
- 不增加按请求选择的 drop 开关、兼容模式、任意字符串过滤器、动态 Provider DSL 或远端元数据同步。
- 不扩展 Chat/Responses Bridge 的工具、structured output、媒体、continuation 或 reasoning 转换能力。
- 不修改 Embeddings 的独立严格 request union，也不处理 MiMo 音频模型的专用参数。
- 不通用校验消息内容、JSON Schema property、工具参数 schema 或其他业务嵌套对象中的任意键。
- 不改变 Native 上游响应/SSE 未知字段的现有处理，不在本焦点修复无关 Provider、GPT 或 NVIDIA 运行缺陷。
- 不把 deterministic mock、SDK loopback 或一次真实 Provider 成功描述为全模型、负载或生产兼容性证明。

## 验证范围

先运行聚焦验证：

```powershell
cargo test --locked --test ingress_contract
cargo test --locked --test capability_definition_contract
cargo test --locked --test bridge_conversion_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test config_contract
```

聚焦测试通过后运行 Rust 基线：

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

确定性测试必须证明字段在实际 transport request 中存在或缺失，而不仅是 planner 返回成功。随后使用真实下游用户 key 运行现有
E2E 入口，至少覆盖 Kimi K3 的 Chat Native 与 Responses Bridge、`stream: true/false` 和一个被忽略的 `temperature`；在不打印
credential、完整请求正文或敏感响应内容的前提下记录最终结果。受参数收窄影响的 GPT 路径放在非 GPT 验收之后执行；NVIDIA 仅在本次
代码实际改变其参数契约时复测。Embeddings、MiMo 音频、负载和长时间运行不属于本焦点验收。

若未修改 `testdata/` 或 `tools/corpus/`，不运行 Python corpus 基线；若实现确实跨越该边界，则按仓库规则补跑对应 `uv` 检查并记录。
