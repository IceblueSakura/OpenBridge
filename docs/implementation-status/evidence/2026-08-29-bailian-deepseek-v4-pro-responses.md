# 2026-08-29 Bailian DeepSeek V4 Pro Responses 接入验证

## 范围与来源

- 时间：2026-08-29 18:05–18:08 CST（UTC+08:00）
- 本地源码基线：`f32f774faee4ada340e9e03ed6dc9f8e03268144` 加本任务未提交修改
- Upstream Target：`bailian/deepseek-v4-pro`
- Upstream model：`deepseek-v4-pro-0813`
- 下游 Public Model：`deepseek-v4-pro`
- endpoint/credential：由 checked-in Target 和私有 credential pool 选择；未输出 endpoint、credential 或 Provider request ID
- 输入：仅固定短合成文本

Alibaba Cloud 当前官方 DeepSeek 文档明确声明 `deepseek-v4-pro` 与 `deepseek-v4-pro-0813` 在华北 2（北京）和新加坡支持 OpenAI-compatible Responses API。本次 Target 使用北京 Provider instance。

## 根因与修复

Public Model 的 Bailian Route 已选择 `DualProtocolNativeOnly`，但 `src/providers/bailian/registration.rs::chat_target()` 只为 Qwen canonical models 创建 Responses `UpstreamApi`，导致 production registry 编译时报：

```text
UnknownReference: bailian/deepseek-v4-pro/responses
```

修复将 `deepseek::deepseek_v4_pro::ID` 纳入已确认的 Responses 静态注册分支。它继续继承保守执行合同：

- stateless / `store:false`；
- `background=false`；
- 不公开 structured output；
- function tools 可用，但不公开可精确控制 `parallel_tool_calls`；
- 不公开 hosted tools。

`deepseek-v4-flash-0731` 的现有 Chat-only Target未被顺带扩展。

## 真实 Provider probe

仓库内 `openbridge-probe` 通过受信 Target、adapter、credential 和 timeout 执行：

### Non-streaming Responses

- HTTP 200；
- `application/json`；
- terminal：`response.completed`；
- usage、reasoning 与 output text 均存在。

### Streaming Responses

- HTTP 200；
- `text/event-stream`；
- terminal：`response.completed`；
- usage、reasoning 与 output text 均存在；
- 观察到 created/in_progress、reasoning delta/done、output text delta/done 与 completed 等正常事件。

## 本地下游 SDK probe

修改后的 OpenBridge 以独立 `127.0.0.1:18080` 启动，复用私有配置但使用 `/tmp` Bootstrap 和日志目录，不影响现有服务。Hindsight 0.9.2 容器内 `openai==2.24.0` 通过本地 `/v1/responses` 调用：

```text
object=response
status=completed
model=deepseek-v4-pro-0813
output_text_observed=true
usage_present=true
```

临时实例已在验证后停止。

## 确定性验证

- 新增回归确认 Pro Target 同时拥有 Chat/Responses，并确认 Flash 0731 未被扩展；
- `cargo test --locked --lib`：PASS；
- `config_contract`：27 passed；
- `example_config`：1 passed；
- `provider_contract`：5 passed。

## 未证明范围

- 未执行 function tool、续轮、structured-output 冲突差分、stateful continuation 或 hosted tool；
- 未验证 reasoning 档位差分、长上下文、429/retry、负载、费用或长期稳定性；
- 本次使用未提交本地二进制，不证明当前生产 `llmapi.icebluesakura.xyz` 已部署该修复。

## 来源

- Alibaba Cloud DeepSeek API：<https://help.aliyun.com/zh/model-studio/deepseek-api>
