# 当前开发焦点

## 状态

**进行中：Operation/capability 重构阶段 4B1——operation-owned Embeddings pipeline。**

## 当前焦点

### 可观察行为

- Embeddings analysis、preflight 与 planning 由 `pipeline/embeddings/` 唯一拥有；crate-level pipeline facade API 保持不变。
- 本切片不移动 I/O response commit；request wire、preflight error、Route plan 与 response behavior 不变。

### 需求与测试

- 需求来源：已批准的 capability-operation-refactor 阶段 4 实施顺序第 2 项。
- 不新增测试；先让 pipeline facade 指向尚不存在的 operation owner 并确认 compile RED，再复用完整 Embeddings contracts。

### 非目标

- 本切片不合并 forwarding loop，不移动 response/commit I/O，不改变 types、retry policy、limits、Provider adapter 或 wire。

### 验证边界

- 运行 Embeddings forwarding/config focused tests，再运行完整 Rust 基线与 `git diff --check`。
- 不读取私有 credential，不运行真实 Provider、外部 SDK、负载或长期测试。