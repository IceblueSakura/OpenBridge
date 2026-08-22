# 当前开发焦点

## 状态

**已规划、待实施：OpenAI Images 标准字段补全与 DashScope 显式扩展。**

本文是下一短周期焦点的执行计划，不表示实现已经开始或完成。实施必须从本文规定的 RED contract tests 开始。

## 目标与成功条件

下游 `POST /v1/images/generations` 以当前 OpenAI Images Create 协议为主合同：标准字段先完成语法分析、model-bound capability preflight 与标准错误分类，再由 Bailian adapter 映射到受信 DashScope native wire。DashScope 专有字段只作为显式、逐模型声明的顶层扩展，不能改变同名 OpenAI 字段的语义，也不能由未知字段 passthrough 自动放行。

完成后：

1. OpenAI SDK 可通过标准参数调用 qwen Images；DashScope 扩展可通过 SDK `extra_body` 合并到同一顶层 JSON；
2. 已知 OpenAI 字段即使当前 qwen profile 不支持，也应返回字段级 `unsupported_model_capability`，而不是被误判为未知字段；
3. adapter 只产生本文定义的 DashScope 字段，所有默认值均显式冻结，不依赖上游隐式默认漂移；
4. `stream:true`、`b64_json`、自动 retry 与异步 task 不得被伪装为已支持。

## 当前基线

- 当前 strict catalog 只有 `model`、`prompt`、`n`、`size`、`response_format`、`user`；见 `src/pipeline/images/analysis.rs::analyze_images_request`。
- 当前 capability 只表达 output count、显式 size 域、URL response format 与一个字符串参数集合；见 `src/core/capability/images.rs::ImagesGenerationsCapabilities`。
- 当前 Bailian adapter 已实现 `prompt`、`n`、`size` 的 DashScope native 转换，并丢弃 `user`、`response_format`；见 `src/providers/bailian/definition.rs::transform_images_request_body`。
- 当前 qwen profile 单 candidate、单 attempt、无 stream/retry/fallback；这一安全边界本焦点保持不变。
- OpenAI 当前 Images Create 标准字段包括 `background`、`model`、`moderation`、`n`、`output_compression`、`output_format`、`partial_images`、`prompt`、`quality`、`response_format`、`size`、`stream`、`style`、`user`。OpenAI Images stream 使用 `image_generation.partial_image` / `image_generation.completed` SSE grammar。
- DashScope qwen-image-3.0 扩展字段包括 `prompt_extend`、`prompt_extend_mode`、`enable_thinking`、`negative_prompt`、`seed`、`watermark`。

## 可观察合同

### 1. OpenAI 标准字段目录

所有标准可选字段接受省略或 JSON `null`；`null` 等价于省略。OpenBridge 仍要求显式非空 `model`，因为不存在默认 Public Model。

| 标准字段 | qwen-image-3.0 / Pro 合同 |
|---|---|
| `prompt` | 必填非空 string，保持当前映射 |
| `n` | 1–6，省略默认 1 |
| `size` | `auto` 或 `WIDTHxHEIGHT`；`auto` 转为省略 DashScope size，显式尺寸转为 `WIDTH*HEIGHT` 并受当前 profile 域约束 |
| `user` | 可选 string；接受但不出网、不进入日志或 metric label |
| `response_format` | 仅 `url`；`b64_json` 返回字段级 `unsupported_model_capability` |
| `output_format` | 仅 `png`；作为已验证的固定输出约束，不作为任意格式转换请求 |
| `stream` | 省略/`null`/`false` 为非流式；`true` 返回字段级 `unsupported_model_capability` |
| `background` | 识别标准类型和值，但 qwen profile 不支持，非空值返回字段级拒绝 |
| `moderation` | 同上，不冒充 OpenAI moderation |
| `output_compression` | 同上，不执行网关下载或重编码 |
| `partial_images` | 同上；不能脱离 `stream:true` 独立生效 |
| `quality` | 同上，不把 DashScope thinking/prompt extension 伪装成 OpenAI quality |
| `style` | 同上，不把 prompt 扩写模式伪装成 OpenAI style |

标准字段必须先通过各自标准类型/枚举/范围验证：形状非法返回 `invalid_request_error`；形状合法但 profile 不支持返回 `unsupported_model_capability`。未知顶层字段仍返回 `invalid_request_error` 且 zero egress。

### 2. DashScope 顶层扩展

为兼容 OpenAI SDK `extra_body`，以下字段按 DashScope 原名作为显式顶层扩展，而不是包装在 `dashscope` 对象中：

| 扩展字段 | 合同 |
|---|---|
| `prompt_extend` | boolean；省略时显式解析为 DashScope-compatible 默认 `true` |
| `prompt_extend_mode` | `direct` / `agent`；省略默认 `direct`；仅在 `prompt_extend=true` 时允许 |
| `enable_thinking` | boolean；省略默认 `true`；仅在 `prompt_extend=true` 时允许 |
| `negative_prompt` | 可选非空 string，直接映射到 DashScope parameters |
| `seed` | integer `[0, 2147483647]`，直接映射；不据此宣称计费幂等或开放 retry |
| `watermark` | boolean；省略默认 `false` |

扩展字段必须进入 typed capability/profile、Public Models 扩展投影和 Provider ceiling containment；不得仅在 adapter 中读取任意 JSON。非 Bailian/qwen Images profile 收到这些字段时返回 `unsupported_model_capability`。

### 3. 响应

- 非流式成功继续返回标准 `{created, data:[{url}]}`；不下载 URL、不合成 `b64_json`。
- 在 DashScope success body 已验证对应事实时，可同时投影标准可选字段 `output_format:"png"` 与 `size:"WIDTHxHEIGHT"`；不得凭请求值伪造响应事实。
- DashScope `usage.output_image_count`、`output_width`、`output_height` 应在完整验证后用于低基数 Images usage/尺寸观测；图片 URL、prompt、negative prompt、seed 和完整上游 body 不进入普通遥测。
- DashScope `rewrite_status` 仅用于内部验证或诊断，不伪装成 `revised_prompt`；上游未返回改写后的文本时不得生成该字段。

## RED 测试边界

先扩展 `tests/images_forwarding_contract.rs`，让以下测试在实现前失败：

1. 标准字段目录接受 `null`、`size:"auto"`、`output_format:"png"`、`stream:false`，并产生精确受信 egress；
2. `stream:true`、`b64_json`、非 PNG output、quality/style/background/moderation/compression/partial-images 的合法值返回对应字段的 `unsupported_model_capability` 且 zero egress；
3. 标准字段类型或枚举非法返回 `invalid_request_error`，与 capability 拒绝可区分；
4. 六个 DashScope 扩展映射到 `parameters`，省略时显式产生已冻结默认值；
5. 扩展依赖错误（如 `prompt_extend=false` 同时指定 mode/thinking）、seed 越界和空 negative prompt 在 egress 前拒绝；
6. 不声明 DashScope extension capability 的 synthetic Images profile 对扩展字段 fail closed；
7. validated success body 投影 `output_format`/`size`，usage 与实际 data 数量或宽高矛盾时 fail closed；
8. OpenAPI schema 与 router contract 仍保持 JSON-only、认证、body limit 和标准错误 envelope。

如不新增 Python `openai` 依赖，则用已有 Rust loopback contract 证明 HTTP wire；本焦点不为单个 SDK 用例修改 `tools/corpus/pyproject.toml`。真实 Provider 验证在确定性测试全绿后单独执行，并只使用既有私有 credential snapshot。

## 实施顺序

1. **需求/OpenAPI 同步**：先更新 Images 功能需求与 OpenAPI request/response schema，记录标准字段和 DashScope extension 分层；不声称 stream 已实现。
2. **RED analysis/preflight tests**：加入标准 known-but-unsupported 分类、`null`/`auto`、extension 类型/依赖和 zero-egress 用例。
3. **Typed capability 扩展**：以结构化 enum/domain 替换单一字符串参数集合对新字段的表达；标准能力与 DashScope extension capability 分开拥有，保持 conservative intersection 与 Provider ceiling containment。
4. **Analyzer/preflight**：解析标准 facts 与 DashScope extension facts；analyzer 不解析 registry，preflight 不选 Route。
5. **Bailian adapter**：将冻结后的标准/扩展字段转换为唯一受信 DashScope envelope；显式发送默认值，移除所有仅属于下游的标准字段。
6. **响应/观测**：验证 DashScope usage 与 data 一致性，投影可证明的标准 response metadata，增加低基数 Images usage 记录。
7. **收口**：更新 implementation status、Bailian status/reference，确认没有保留旧字符串 allowlist 双路径；完成后把本文恢复空闲态。

## 明确非目标

- 不实现 `stream:true`、partial-image SSE、DashScope async task 创建/轮询或任何伪流式包装；
- 不实现 `b64_json`、图片下载、转码、压缩、缓存或 URL 延寿；
- 不实现 Images edits/variations、I2I 或输入图片；
- 不增加自动 retry、credential rotation、fallback 或把 seed 当作 idempotency key；
- 不把 `quality`/`style`/`background`/`moderation` 映射到语义不同的 DashScope 字段；
- 不改变现有 qwen Images single-candidate/single-attempt 合同；
- 不修改私有配置、credential 文件或 checked-in development logging policy。

## 验证与完成门

按顺序执行：

```powershell
cargo test --locked --test images_forwarding_contract
cargo test --locked --test ingress_contract
cargo test --locked --test observability_contract
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

文档改动另跑全仓 Markdown 相对链接/锚点检查和 OpenAPI YAML 解析。真实 DashScope 验证至少覆盖：标准-only 请求、完整 extension 请求、`size:auto`、`output_format:png`、一个 known-but-unsupported 标准字段的 zero-egress。禁止在输出中打印 credential、prompt、negative prompt 或签名 URL。

完成条件：所有确定性检查通过；真实验证结果按证据边界记录；需求、OpenAPI、实现、Models 投影、测试和状态文档一致；`current-focus.md` 恢复空闲态。

## 风险与回滚

- 扩展 strict catalog 会把过去的“未知字段”改为更精确的“已知但模型不支持”，属于预期错误分类变化；成功合同不应回退。
- 显式冻结 DashScope 默认值会稳定行为，但可能改变未来上游默认更新后的结果；这是有意的可重复性选择。
- 若真实 Provider 显示文档字段或默认值与 wire 不一致，停止扩大 capability，保留 RED/zero-egress，并将差异记录为未证明，不添加兼容 shim。
- 回滚单位是本焦点整体：标准字段目录、typed capability、adapter 和文档必须一起回滚，不能留下 parser 接受但 adapter 静默丢弃的半实现状态。
