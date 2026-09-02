# 2026-09-02 四模型 Chat/Responses 双协议能力探测矩阵

## 记录类型

**接入验证记录 + 差异记录**。本记录来自一次真实执行的管理员能力探测矩阵，覆盖四个已注册
Generation Target 在 Chat 与 Responses 双协议、JSON 与 SSE 双交付下的固定闭合 case。部分结果与
当前注册声明不一致，按仓库规则同时保留差异。

## 执行边界

- **工具**：`target/release/openbridge-probe`（commit `77037f2`，含 3 个 Responses-only 单字段差分
  case：`reasoning-summary` / `include-encrypted-content` / `prompt-cache-key`），由
  `tools/probe/matrix.py` 编排，逐调用独立进程、独立脱敏报告。
- **矩阵**：4 Target × (Chat 19 case + Responses 22 case) × 2 delivery = **328 次真实请求**，
  全部在本机对真实 Provider 执行，调用间隔 2s，单次超时 300s。
- **固定 payload**：text/reasoning/structured/tool/image case 使用 probe 内置固定 prompt、schema、
  工具定义与图片；无自定义覆盖。
- **凭证/账号/区域**：使用 `config/upstream-credentials.toml` 当前激活池（单一账号），与运行服务
  相同的静态受信出口。
- **原始报告**：`testdata/runtime/probe-2026-09-02/`（gitignored），含 `plan.json`、逐调用
  `<model>_<protocol>_<case>_<json|sse>.json`、`results.jsonl`、`summary.json`、`matrix.md`。

## 不证明什么

单账号、单区域、固定 payload、单次执行。不证明长期稳定性、质量、其他账号/区域、负载、
外部 SDK/Agent 行为，也不证明被拒绝项在其它模型或未来 Provider 版本下仍被拒绝。
`accepted` 仅表示该固定请求被接受，`not_honored` 表示接受但输出未匹配固定 oracle。

## 模型与协议覆盖

| 模型 | Provider | Target | upstream model | Chat | Responses |
|---|---|---|---|---|---|
| deepseek-v4-flash-vision-exp | DeepSeek | `deepseek-v4-flash-vision-exp` | `deepseek-v4-flash-vision-exp` | ✓ | ✓ |
| mimo-v2.5 | Xiaomi MiMo | `mimo-v2-5` | `mimo-v2.5` | ✓ | ✓ |
| glm-5.3-flash | Zhipu AI China | `zhipu-cn/glm-5-3-flash` | `glm-5.3-flash` | ✓ | ✓ |
| qwen3.8-max | Bailian | `bailian/qwen3-8-max` | `qwen3.8-max` | ✓ | ✓ |

## 结果汇总

图例：`OK`=accepted+supported；`NH`=accepted+not_honored；`INC`=accepted+inconclusive；
`REJ`=rejected（HTTP 4xx）；`REJ/UTF`=HTTP 200 但 SSE 终态 `response.failed`。JSON/SSE 两列
除特别标注外一致。完整逐 case 表见 `testdata/runtime/probe-2026-09-02/matrix.md`。

### deepseek-v4-flash-vision-exp

- text / 全部 7 档 reasoning / image-input：Chat 与 Responses 均 `OK`。
- json-object：Chat `NH`；Responses `NH`。
- json-schema / json-schema-strict：Chat `REJ`（注册 Chat 为 JsonObject，一致）；Responses `NH`
  （注册 Responses 为 NonStrictOnly，接受但不强制，一致）。
- **tool-required / tool-named / tool-strict / tool-parallel-false / tool-parallel-true：
  Chat 与 Responses 均 `REJ`（400）。** 仅 tool-auto / tool-none `OK`。
- reasoning-summary / include-encrypted-content / prompt-cache-key：Responses 均 `OK`。

### mimo-v2.5

- text / reasoning-none..xhigh / json-object / image-input：双协议 `OK`。
- **reasoning-max：Responses `REJ`（400）**（Chat `OK`）。
- json-schema / json-schema-strict：Chat `OK`；**Responses `REJ`（400）**（注册 Responses
  `structured_outputs: None`，一致）。
- tool-none / tool-named / tool-strict / tool-parallel-false：双协议 `NH`（接受但未按固定 oracle
  调用）；tool-auto / tool-required / tool-parallel-true `OK`。
- reasoning-summary / include-encrypted-content / prompt-cache-key：Responses 均 `OK`。

### glm-5.3-flash

- text / image-input：双协议 `OK`。
- **reasoning effort 仅接受子集**：`low` / `high` / `max` `OK`；
  `none` / `minimal` / `medium` / `xhigh` Chat `REJ`（400），Responses `REJ` 且 SSE 侧为
  `REJ/UTF`（`response.failed`）。
- json-object：Chat JSON `NH` / SSE `OK`；json-schema / json-schema-strict：双协议 `NH`。
- tool-none / tool-named / tool-strict：双协议 `NH`；tool-auto / tool-required /
  tool-parallel-* `OK`。
- **reasoning-summary：Responses `REJ`（JSON 400，SSE `response.failed`）。**
- **include-encrypted-content：Responses `REJ`。** prompt-cache-key：Responses `OK`。

### qwen3.8-max

- text / 全部 7 档 reasoning / image-input：双协议 `OK`。
- json-object / json-schema / json-schema-strict：Chat `OK`（json-object JSON 侧 `NH`）；
  Responses 均 `NH`（注册 Responses 为 JsonObject，接受但不强制）。
- Chat tool-named `NH`，tool-strict JSON `OK`/SSE `NH`，tool-parallel-false SSE `INC`，
  tool-parallel-true JSON `NH`/SSE `OK`；其余 `OK`。
- **Responses tool-required / tool-parallel-false / tool-parallel-true：`REJ`（JSON 400，
  SSE `response.failed`）**；tool-named `NH`；tool-strict `OK`；tool-auto / tool-none `OK`。
  （注册 `RESPONSES_TOOL_CHOICE_MODES=[None,Auto]`，与实测吻合。）
- reasoning-summary：Responses JSON `OK` 且 `reasoning_summary_observed=true`，SSE `OK` 但
  summary 事件未观测到。include-encrypted-content / prompt-cache-key：Responses 均 `OK`。

## 与注册声明的差异（待收窄确认）

以下为实测与当前注册不一致、值得在独立获准变更中复核的点（本记录只固定事实，不改注册）：

1. **deepseek-v4-flash-vision-exp**：注册对该 Target 声明 `ALL_TOOL_CHOICE_MODES`
   （含 Required/Named），但 Chat 与 Responses 实测仅接受 auto/none；required/named/strict/
   parallel 全部 400。疑似 vision 模型不支持强制工具选择，注册对该模型过宽。
2. **glm-5.3-flash**：reasoning effort 实测仅 `low/high/max` 三档被接受；
   `none/minimal/medium/xhigh` 被拒绝。注册未体现该 effort 子集收窄。
   同时 Responses `reasoning-summary` 与 `include-encrypted-content` 被拒绝。
3. **mimo-v2.5**：Responses `reasoning-max` 被 400 拒绝（Chat 接受），注册未区分协议维度的
   reasoning 上限。

## 复现

```powershell
python3 tools/probe/matrix.py --out testdata/runtime/probe-2026-09-02 \
  --delay-seconds 2 --timeout-seconds 300
# 或限定子集：--target-label qwen3.8-max --protocol responses --case reasoning-summary
```
