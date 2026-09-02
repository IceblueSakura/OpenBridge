# 2026-09-02 双协议能力探测记录（持续追加）

## 记录类型

**接入验证记录 + 差异记录**。本记录保存管理员能力探测矩阵的逐轮真实执行结果。每轮探测以
独立小节追加；探测全部完成后再统一修改注册与文档，期间只固定事实、不改注册。部分结果与
当前注册声明不一致，按仓库规则同时保留差异。

## 第 1 轮：四模型 Chat/Responses 双协议全矩阵（2026-09-02）

### 执行边界

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

### 不证明什么

单账号、单区域、固定 payload、单次执行。不证明长期稳定性、质量、其他账号/区域、负载、
外部 SDK/Agent 行为，也不证明被拒绝项在其它模型或未来 Provider 版本下仍被拒绝。
`accepted` 仅表示该固定请求被接受，`not_honored` 表示接受但输出未匹配固定 oracle。

### 模型与协议覆盖

| 模型 | Provider | Target | upstream model | Chat | Responses |
|---|---|---|---|---|---|
| deepseek-v4-flash-vision-exp | DeepSeek | `deepseek-v4-flash-vision-exp` | `deepseek-v4-flash-vision-exp` | ✓ | ✓ |
| mimo-v2.5 | Xiaomi MiMo | `mimo-v2-5` | `mimo-v2.5` | ✓ | ✓ |
| glm-5.3-flash | Zhipu AI China | `zhipu-cn/glm-5-3-flash` | `glm-5.3-flash` | ✓ | ✓ |
| qwen3.8-max | Bailian | `bailian/qwen3-8-max` | `qwen3.8-max` | ✓ | ✓ |

### 结果汇总

图例：`OK`=accepted+supported；`NH`=accepted+not_honored；`INC`=accepted+inconclusive；
`REJ`=rejected（HTTP 4xx）；`REJ*`=HTTP 200 但 SSE 终态 `response.failed`。单元格出现
`JSON/SSE` 组合（如 `NH/OK`）表示两种交付结果不同；单一值表示两种交付一致。逐 case 全表见
[完整矩阵](#完整逐-case-矩阵第-1-轮)；逐调用原始脱敏报告在 `testdata/runtime/probe-2026-09-02/`
（gitignored）。

#### deepseek-v4-flash-vision-exp

- text / 全部 7 档 reasoning / image-input：Chat 与 Responses 均 `OK`。
- json-object：Chat `NH`；Responses `NH`。
- json-schema / json-schema-strict：Chat `REJ`（注册 Chat 为 JsonObject，一致）；Responses `NH`
  （注册 Responses 为 NonStrictOnly，接受但不强制，一致）。
- **tool-required / tool-named / tool-strict / tool-parallel-false / tool-parallel-true：
  Chat 与 Responses 均 `REJ`（400）。** 仅 tool-auto / tool-none `OK`。
- reasoning-summary / include-encrypted-content / prompt-cache-key：Responses 均 `OK`。

#### mimo-v2.5

- text / reasoning-none..xhigh / json-object / image-input：双协议 `OK`。
- **reasoning-max：Responses `REJ`（400）**（Chat `OK`）。
- json-schema / json-schema-strict：Chat `OK`；**Responses `REJ`（400）**（注册 Responses
  `structured_outputs: None`，一致）。
- tool-none / tool-named / tool-strict / tool-parallel-false：双协议 `NH`（接受但未按固定 oracle
  调用）；tool-auto / tool-required / tool-parallel-true `OK`。
- reasoning-summary / include-encrypted-content / prompt-cache-key：Responses 均 `OK`。

#### glm-5.3-flash

- text / image-input：双协议 `OK`。
- **reasoning effort 仅接受子集**：`low` / `high` / `max` `OK`；
  `none` / `minimal` / `medium` / `xhigh` Chat `REJ`（400），Responses `REJ` 且 SSE 侧为
  `REJ*`（`response.failed`）。
- json-object：Chat JSON `NH` / SSE `OK`；json-schema / json-schema-strict：双协议 `NH`。
- tool-none / tool-named / tool-strict：双协议 `NH`；tool-auto / tool-required /
  tool-parallel-* `OK`。
- **reasoning-summary：Responses `REJ`（JSON 400，SSE `response.failed`）。**
- **include-encrypted-content：Responses `REJ`。** prompt-cache-key：Responses `OK`。

#### qwen3.8-max

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

### 完整逐 case 矩阵（第 1 轮）

以下表格为第 1 轮 328 次探测的逐 case 结果。每行一个 case，单元格按图例给出结论；形如 `NH/OK` 的单元格表示 JSON/SSE 两种交付结果不同（前者为 JSON、后者为 SSE）。
#### deepseek-v4-flash-vision-exp — chat

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | OK |
| reasoning-minimal | OK |
| reasoning-low | OK |
| reasoning-medium | OK |
| reasoning-high | OK |
| reasoning-xhigh | OK |
| reasoning-max | OK |
| json-object | NH |
| json-schema | REJ |
| json-schema-strict | REJ |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | OK |
| tool-required | REJ |
| tool-named | REJ |
| tool-strict | REJ |
| tool-parallel-false | REJ |
| tool-parallel-true | REJ |

#### deepseek-v4-flash-vision-exp — responses

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | OK |
| reasoning-minimal | OK |
| reasoning-low | OK |
| reasoning-medium | OK |
| reasoning-high | OK |
| reasoning-xhigh | OK |
| reasoning-max | OK |
| json-object | NH |
| json-schema | NH |
| json-schema-strict | NH |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | OK |
| tool-required | REJ |
| tool-named | REJ |
| tool-strict | REJ |
| tool-parallel-false | REJ |
| tool-parallel-true | REJ |
| reasoning-summary | OK |
| include-encrypted-content | OK |
| prompt-cache-key | OK |

#### mimo-v2.5 — chat

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | OK |
| reasoning-minimal | OK |
| reasoning-low | OK |
| reasoning-medium | OK |
| reasoning-high | OK |
| reasoning-xhigh | OK |
| reasoning-max | OK |
| json-object | OK |
| json-schema | OK |
| json-schema-strict | OK |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | NH |
| tool-required | OK |
| tool-named | NH |
| tool-strict | NH |
| tool-parallel-false | NH |
| tool-parallel-true | OK |

#### mimo-v2.5 — responses

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | OK |
| reasoning-minimal | OK |
| reasoning-low | OK |
| reasoning-medium | OK |
| reasoning-high | OK |
| reasoning-xhigh | OK |
| reasoning-max | REJ |
| json-object | OK |
| json-schema | REJ |
| json-schema-strict | REJ |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | NH |
| tool-required | OK |
| tool-named | NH |
| tool-strict | NH |
| tool-parallel-false | NH |
| tool-parallel-true | OK |
| reasoning-summary | OK |
| include-encrypted-content | OK |
| prompt-cache-key | OK |

#### glm-5.3-flash — chat

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | REJ |
| reasoning-minimal | REJ |
| reasoning-low | OK |
| reasoning-medium | REJ |
| reasoning-high | OK |
| reasoning-xhigh | REJ |
| reasoning-max | OK |
| json-object | NH/OK |
| json-schema | NH |
| json-schema-strict | NH |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | NH |
| tool-required | OK |
| tool-named | NH |
| tool-strict | NH |
| tool-parallel-false | NH |
| tool-parallel-true | OK |

#### glm-5.3-flash — responses

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | REJ/REJ* |
| reasoning-minimal | REJ/REJ* |
| reasoning-low | OK |
| reasoning-medium | REJ/REJ* |
| reasoning-high | OK |
| reasoning-xhigh | REJ/REJ* |
| reasoning-max | OK |
| json-object | NH |
| json-schema | NH |
| json-schema-strict | NH |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | NH |
| tool-required | OK |
| tool-named | NH |
| tool-strict | NH |
| tool-parallel-false | NH |
| tool-parallel-true | OK |
| reasoning-summary | REJ/REJ* |
| include-encrypted-content | REJ/REJ* |
| prompt-cache-key | OK |

#### qwen3.8-max — chat

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | OK |
| reasoning-minimal | OK |
| reasoning-low | OK |
| reasoning-medium | OK |
| reasoning-high | OK |
| reasoning-xhigh | OK |
| reasoning-max | OK |
| json-object | NH/OK |
| json-schema | OK |
| json-schema-strict | OK |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | OK |
| tool-required | OK |
| tool-named | NH |
| tool-strict | OK/NH |
| tool-parallel-false | OK/INC |
| tool-parallel-true | NH/OK |

#### qwen3.8-max — responses

| case | JSON | SSE |
|---|---|---|
| text | OK |
| reasoning-none | OK |
| reasoning-minimal | OK |
| reasoning-low | OK |
| reasoning-medium | OK |
| reasoning-high | OK |
| reasoning-xhigh | OK |
| reasoning-max | OK |
| json-object | NH |
| json-schema | NH |
| json-schema-strict | NH |
| image-input-inline-png | OK |
| tool-auto | OK |
| tool-none | OK |
| tool-required | REJ/REJ* |
| tool-named | NH |
| tool-strict | OK |
| tool-parallel-false | REJ/REJ* |
| tool-parallel-true | REJ/REJ* |
| reasoning-summary | OK |
| include-encrypted-content | OK |
| prompt-cache-key | OK |

### 与注册声明的差异（待收窄确认，探测完成后统一处理）

以下为实测与当前注册不一致、值得在独立获准变更中复核的点（本记录只固定事实，不改注册）：

1. **deepseek-v4-flash-vision-exp**：注册对该 Target 声明 `ALL_TOOL_CHOICE_MODES`
   （含 Required/Named），但 Chat 与 Responses 实测仅接受 auto/none；required/named/strict/
   parallel 全部 400。疑似 vision 模型不支持强制工具选择，注册对该模型过宽。
2. **glm-5.3-flash**：reasoning effort 实测仅 `low/high/max` 三档被接受；
   `none/minimal/medium/xhigh` 被拒绝。注册未体现该 effort 子集收窄。
   同时 Responses `reasoning-summary` 与 `include-encrypted-content` 被拒绝。
3. **mimo-v2.5**：Responses `reasoning-max` 被 400 拒绝（Chat 接受），注册未区分协议维度的
   reasoning 上限。

### 复现

```powershell
python3 tools/probe/matrix.py --out testdata/runtime/probe-2026-09-02 \
  --delay-seconds 2 --timeout-seconds 300
# 或限定子集：--target-label qwen3.8-max --protocol responses --case reasoning-summary
```

## 后续轮次

尚未执行。后续探测完成后在本文件追加"第 2 轮"等小节；全部轮次完成后再统一提交注册与文档修改。
