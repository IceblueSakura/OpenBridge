# 当前架构

当前 crate 是 **v2 语义库**，不含网关运行时或 binary target。[v2 设计](architecture-v2/README.md)描述目标结构；尚未落地的 topology、execution、credential 与 ingress 不应据此被视为已实现。

```text
JSON / SSE payload
 → protocol decode
 → semantic Generation request / response / event
 → validation / trusted semantic transform
 → requirements / lowering
 → protocol encode
 → JSON / SSE payload
```

| Owner | 当前责任 |
|---|---|
| `src/semantic/` | 有界值、Generation ordered items、工具/reasoning/文本控制、Static/Event、语义验证与 requirements；不依赖 HTTP、Provider 或 registry |
| `src/protocol/` | Chat/Responses 语法、bounded fidelity、完整 envelope 与 Responses SSE adapter；不访问网络、私有配置或 credential |
| `src/lowering/` | 从不可变最终 IR 验证固定目标表示；不选择 Provider，不恢复已删除源值 |
| `src/transport/sse.rs` | 有界字节 framing；strict EOF 与 permissive EOF 调用边界分开 |
| `tests/semantic_v2_*` | 独立语义、变换与失败边界；测试专用 HTTP/SDK 使用临时 loopback 和 synthetic 数据 |

`src/lib.rs` 只公开上述四个模块。Axum、Tokio、reqwest 等只作为测试依赖；库没有认证、配置加载、Provider HTTP client、重试或服务监听入口。现存私人配置不被库或测试读取。

Responses 纯文本 profile 的具体准入见[专页](architecture-v2/responses-text-profile.md)。现有资源类型不代表媒体 codec 已接通，Generation 验收也不代表其他任务已支持。

旧 IR/Bridge/pipeline、ToolPlan/gateway-tools 原型及整个旧运行时在 [Git 归档](archive.md)中；退役不构成功能迁移完成。后续实现按旧行为的独立证据逐项决定是否迁移，不机械搬回目录。
