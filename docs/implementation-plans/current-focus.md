# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 4B2——pure Embeddings response driver。**

## 当前焦点

### 可观察行为

- Embeddings success media type、JSON contract、vector shape、model projection 与 usage extraction 由 `pipeline/embeddings/response.rs` 纯函数拥有。
- ingress 只执行 bounded body read、observation 与 downstream Response commit；response wire 与错误行为不变。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 4 operation response driver 边界。
- 不新增测试；先让 ingress 引用尚不存在的 pure response API 并确认 compile RED，再复用完整 Embeddings contracts。

### 非目标

- 本切片不合并 forwarding loop，不移动 body I/O 或 commit，不改变 retry policy、limits、Provider adapter 或 wire。

### 验证边界

- 运行 Embeddings forwarding/config focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。