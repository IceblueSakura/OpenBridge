# Images Generations 能力需求

本文只定义下游 `POST /v1/images/generations` 的固定行为、失败语义、安全边界与非目标；实现与验证事实统一见
[实施现状](../../implementation-status/README.md)。

## 1. 下游契约

- endpoint 为 `POST /v1/images/generations`，JSON-only，同一静态下游 Bearer 认证。
- strict request catalog 识别当前 OpenAI Images Create 字段：`model`、`prompt`、`n`、`size`、`response_format`、
  `output_format`、`stream`、`partial_images`、`background`、`moderation`、`output_compression`、`quality`、`style`、`user`；
  未知顶层字段在 egress 前以 400 `invalid_request_error` 拒绝。
- 已知标准字段先完成类型/枚举/range 分析，再由 model-bound preflight 判断支持；当前 qwen profile 对不支持的标准字段返回
  400 `unsupported_model_capability` 和准确 `param`，不得降级为 unknown field 或静默丢弃。
- OpenAI optional `null` 视为省略。qwen 支持 `n`、`size: "auto" | "宽x高"`、`response_format: "url"`、
  `output_format: "png"`、`stream:false` 和 `user`；`b64_json`、jpeg/webp、`stream:true`、partial image、quality/style/background/
  moderation/compression 均 fail closed。
- qwen profile 额外识别 DashScope 顶层扩展（兼容 OpenAI SDK `extra_body`）：`prompt_extend`、`prompt_extend_mode`、
  `enable_thinking`、`negative_prompt`、`seed`、`watermark`。扩展必须由 `interfaces.images.dashscope_extensions` 明确公开；
  无该 profile 的模型按字段拒绝。
- 成功响应固定为 `{created, data: [{url}], output_format: "png", size: "宽x高"}`；`data` 数量等于解析后的 `n`，
  size 来自已验证 DashScope usage，URL 是 Provider 短期签名 URL，不构成永久 resource identity。

## 2. 能力契约与预检

- Images 是独立 operation（`images_generations`），不进入 Chat/Responses Bridge，无生成协议语义。
- Provider ceiling 由 `ImagesGenerationsCapabilities` 拥有：`n` 上限、size 域（每边与面积）、
  `response_format` 域、标准参数集合及可选 `DashScopeImagesCapabilities`；Public Model 的 `interfaces.images` 是全部固定候选的
  保守交集，default 必须一致，DashScope extension profile 仅在所有候选完全一致时公开。
- preflight 一次解析标准字段、DashScope extension facts 与响应预期并冻结；超域、未声明字段或冲突依赖在首次 egress 前返回
  400，不能改选模型或 Route。`prompt_extend:false` 与显式 mode/thinking child 冲突，按字段返回 `invalid_request_error`。
- DashScope 默认明确冻结为 `prompt_extend:true`、`prompt_extend_mode:"direct"`、`enable_thinking:true`、
  `watermark:false`；`seed` 为 `[0, 2147483647]`，`negative_prompt` 必须非空白 string。
- ImageGeneration canonical task 固定为 text→image 生成；它不继承 Generation task 的 reasoning、streaming
  或 function-tool 语义。

## 3. 上游 wire 与响应验证

- 每个 Provider adapter 只使用其受信注册的 Native 路径；业务请求不能覆盖 URL、模型、credential 或认证 header。
- OpenAI 请求向 DashScope 原生请求的转换只做已证明映射：`prompt`→`input.messages`、`n`→`parameters.n`、
  `size` 的 `x`→`*`，`size:"auto"` 转为省略；`user`、`response_format`、`output_format`、`stream:false` 不离开网关。
- 已通过 extension preflight 的六个 DashScope 字段才进入 `parameters`；省略字段使用冻结默认，禁止 adapter 接受任意 JSON passthrough。
- 上游响应在 commit 前整体验证：按 body `code` 字段识别业务错误、逐 choice 提取非空图片 URL、`usage.output_image_count`
  与解析后的 `n` 一致、width/height 为正整数、投影后 JSON 不超过 response budget。任何违反 fail closed，不提交部分结果。
- 验证后的图片数量、宽、高写入独立 Images histogram；不得伪装成 token usage。

## 4. Retry、取消与数据保护

- Images generation 不自动 retry/fallback：请求可能已被接受、计费或产生结果，网络不确定时不得盲目重放。
  单 candidate 单 attempt。
- 下游取消终止上游请求；响应提交后不得重放或拼接。
- prompt、上游 body、错误上下文与图片 URL 不进入普通日志、OTLP trace attribute 或 metric label。

## 5. 非目标

- `b64_json` 响应、`stream:true`/partial-image SSE、异步任务轮询（`X-DashScope-Async`）与任务查询；
- Images edit/variation、I2I 编辑、多 Provider/多 Target fallback 与请求期 capability routing；
- 网关下载、缓存、代理或延长 Provider 图片 URL；OCR、内容安全或质量承诺。

## 6. 验收项

- IMG-GEN-01：标准字段目录与未知字段区分；已知但不支持字段按准确 `param` zero-egress；
- IMG-GEN-02：OpenAI `null`、`size:"auto"`、PNG 与 `stream:false` omission-equivalent 路径通过；其余标准域按 model profile 拒绝；
- IMG-GEN-03：DashScope 六字段类型、range、依赖和缺失 extension profile 均 fail closed；冻结默认与显式值准确进入 native wire；
- IMG-GEN-04：成功响应按 choice/usage 双重校验，投影 URL、PNG 和实际 size，图片 count/width/height 进入非 token metrics；
- IMG-GEN-05：单 attempt、无重放；prompt、negative prompt 与 URL 不进入遥测。
