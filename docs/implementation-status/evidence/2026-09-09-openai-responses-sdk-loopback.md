# OpenAI Python SDK Responses 工具续轮验收

## 执行边界

- 日期：2026-09-09，Asia/Shanghai（UTC+08:00）。
- Checkout：`22e3ebbab61cf28a11d8b9392f9674029c3861c1` 加本次未提交的能力收窄、SDK fixture 和资源回归。不是已部署服务验收。
- 客户端：官方 `openai==3.10.0`；版本来自 [PyPI](https://pypi.org/project/openai/3.10.0/)，运行时检查版本。Python 3.14.7、uv 0.12.5；仅固定 SDK 版本，不声称固定全部传递依赖。
- 服务端：Rust production Router + synthetic registry、凭据和 socket-backed mock upstream；上下游都由测试绑定 `127.0.0.1:0`，不读取用户私有配置，不调用真实 Provider。
- 相关实现：`tests/openai_responses_sdk_loopback.rs`、`tests/sdk/openai_responses_tool_loop.py`；运行入口见[开发指南](../../development.md#独立-openai-sdk-responses-验收)。

## 实际验收

通过如下显式命令执行，默认 Rust 基线忽略外部 SDK gate：

```sh
uv run --no-project --with openai==3.10.0 cargo test --locked --test openai_responses_sdk_loopback -- --ignored --nocapture --test-threads=1
```

| 场景 | 结果 |
|---|---|
| Responses JSON 两轮 | 通过：SDK 解析 function call，本地执行 synthetic weather tool，完整历史回传后取得最终回答与 usage |
| Responses SSE 两轮 | 通过：SDK 消费 arguments/text delta 与唯一 completed 终态，组装结果后回传工具历史 |
| 缺失 call_id 负向控制 | SDK 收到网关 400 / `unsupported_model_capability`，上游只有首轮请求；不是 SDK 本地异常或 mock 拒绝 |
| 错误工具结果负向控制 | 上游实际收到第二轮并由固定 oracle 返回 422；oracle 不随负向控制改变期望结果 |

两轮均使用 `store:false`，省略 `previous_response_id`。客户端实际调用固定本地工具，无网络和文件副作用；网关不执行工具。SDK 关闭 retries、环境代理及 redirects，并设置请求超时。测试直接拥有 Python 子进程和 loopback servers，失败或取消时清理。

上游 oracle 独立检查 model 改写、instructions、schema、工具选择、历史顺序、call_id 和结果；SDK 另检查类型化输出、完整 delta、终态及 token usage。它不是用生产 codec 生成自身 expected 的自证。

## 不证明什么

mock upstream 返回固定 synthetic 结果，不证明任何模型会正确选择或执行工具，不证明真实 Provider、Agent runtime、其他 SDK 版本、并行工具、媒体、Bridge 路径、HTTP/2、负载或长期兼容。此次 Native Responses SDK 验收与另行批准的 DeepSeek 首轮 probe 是不同证据，不能合并为真实 Provider 的完整工具续轮验收。
