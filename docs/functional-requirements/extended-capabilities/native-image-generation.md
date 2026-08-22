# Images Generations 能力需求

本文只定义下游 `POST /v1/images/generations` 的固定行为、失败语义、安全边界与非目标；实现与验证事实统一见
[实施现状](../../implementation-status/README.md)。

## 1. 下游契约

- endpoint 为 `POST /v1/images/generations`，JSON-only，同一静态下游 Bearer 认证。
- strict request catalog 只允许 `model`、`prompt`、`n`、`size`、`response_format`、`user`；
  未知顶层字段在 egress 前以 400 `invalid_request_error` 拒绝。
- `prompt` 必须是非空白 string；`n` 是正整数；`size` 是 `宽x高` 十进制像素对；`response_format` 当前只接受
  `url`（省略时默认 `url`），`b64_json` 与其它值拒绝；`user` 为可选 string 且不转发。
- 成功响应固定为 `{created, data: [{url}]}`，`data` 数量等于解析后的 `n`；URL 是 Provider 短期签名 URL，
  不构成永久 resource identity。

## 2. 能力契约与预检

- Images 是独立 operation（`images_generations`），不进入 Chat/Responses Bridge，无生成协议语义。
- Provider ceiling 由 `ImagesGenerationsCapabilities` 拥有：`n` 上限、size 域（每边与面积）、
  `response_format` 域及 `supported_parameters`；Public Model 的 `interfaces.images` 是全部固定候选的保守交集，
  default 必须一致。
- preflight 一次解析 `n`、`size`、`response_format` 并冻结；超域或未声明字段在首次 egress 前返回
  400 `unsupported_model_capability`，不改选模型或 Route。
- ImageGeneration canonical task 固定为 text→image 生成；它不继承 Generation task 的 reasoning、streaming
  或 function-tool 语义。

## 3. 上游 wire 与响应验证

- 每个 Provider adapter 只使用其受信注册的 Native 路径；业务请求不能覆盖 URL、模型、credential 或认证 header。
- OpenAI 请求向 DashScope 原生请求的转换只做已证明字段映射（`prompt`→`input.messages`、`n`→`parameters.n`、
  `size` 的 `x`→`*`）；`user` 与 `response_format` 不离开网关，不得伪造上游语义。
- 上游响应在 commit 前整体验证：按 body `code` 字段识别业务错误、逐 choice 提取非空图片 URL、数量与解析
  `n` 一致、投影后 JSON 不超过 response budget。任何违反 fail closed，不提交部分结果。

## 4. Retry、取消与数据保护

- Images generation 不自动 retry/fallback：请求可能已被接受、计费或产生结果，网络不确定时不得盲目重放。
  单 candidate 单 attempt。
- 下游取消终止上游请求；响应提交后不得重放或拼接。
- prompt、上游 body、错误上下文与图片 URL 不进入普通日志、OTLP trace attribute 或 metric label。

## 5. 非目标

- `b64_json` 响应、stream/progress 输出、异步任务轮询（`X-DashScope-Async`）与任务查询；
- Images edit/variation、I2I 编辑、多 Provider/多 Target fallback 与请求期 capability routing；
- 网关下载、缓存、代理或延长 Provider 图片 URL；OCR、内容安全或质量承诺。

## 6. 验收项

- IMG-GEN-01：strict catalog 与未知字段在 egress 前拒绝；
- IMG-GEN-02：`n`/`size`/`response_format` 超域返回 400 `unsupported_model_capability` 且 zero egress；
- IMG-GEN-03：Native 转换的 egress 请求只含受信映射字段；
- IMG-GEN-04：成功响应按 `n` 校验并投影为 `{created, data: [{url}]}`，数量不匹配 fail closed；
- IMG-GEN-05：单 attempt、无重放；prompt 与 URL 不进入遥测。
