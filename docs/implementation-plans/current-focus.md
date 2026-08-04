# 当前开发焦点

## 可观察行为

已认证客户端向 `POST /v1/embeddings` 提交一个已注册 Embedding Public Model 的 OpenAI-compatible JSON 请求时，OpenBridge 经单条 Native OpenAI-compatible Embeddings Route 改写真实 upstream model，并返回不改变向量编码、维度、顺序、index、model 与 usage 的 JSON 响应。

这是 Embeddings 目标的第一个可执行切片。当前焦点先证明完整协议链可被 registry、Router、Provider adapter 与 transport 表达；具体真实 Provider/model 的 checked-in 注册及真实调用证据不在本切片内。

## 对应需求与参考

- [Embeddings 与 Native 多模态扩展需求](../functional-requirements/embedding-and-native-multimodal.md)
- [Public Model 与模型能力契约](../functional-requirements/model-information-and-capability-contract.md)
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)
- [Embeddings 协议实现细节](../references/openai/implementation-details/01-embeddings.md)

## 先行失败测试

实现前新增或定位以下失败行为：

1. registry 可以表示 Embedding task、Embeddings Upstream API、单条 Native Route 和仅有 Embeddings interface 的 Public Model；协议错配、能力扩大或把 Embedding model 当 Chat/Responses 使用会在启动时失败。
2. 扩展 Models DTO 公开独立 `interfaces.embeddings`，标准 Models 仍严格保持四字段；两者都不暴露真实 model、Provider、Target、Route 或 credential。
3. 受保护的 `POST /v1/embeddings` 接受 string、string batch、token array 和 token-array batch，拒绝非 JSON、非法/空 union、未知 Public Model、stream 与未声明的 `encoding_format`/`dimensions`，且拒绝时 upstream 调用次数为零。
4. Native adapter 只把下游 Public Model 改写为受信 upstream model，不能由 body/header/query 选择 URL、credential 或 Route；合法的 endpoint-specific 字段保持原 wire。
5. 成功 JSON 保持 `object`、`data[]` 顺序、`index`、float/base64 embedding、响应 `model` 与 `usage`；错误 media type、非法/超限成功体安全失败。
6. 同一请求只使用单条 Embeddings Route 的有限 attempt；下游取消停止发送/等待，且本切片不存在跨 Provider/模型 fallback。

## 实现边界

- 为 Embeddings 增加独立 operation、任务模式、接口 capability、请求分析和 OpenAI-compatible endpoint path；不把它塞入生成 capability 或 Chat/Responses Bridge。
- ingress 只接受有界 `application/json`；请求与响应分别有大小限制，正文、token、向量和 base64 不进入日志或 metrics。
- deterministic contract 使用合成 model、向量和 mock upstream，不使用真实凭证或私人文本。
- 对 string 输入不在网关内实现 tokenizer；只有结构、字节和已声明可直接验证的批量/维度/编码限制在 egress 前校验，Provider token 限制由真实 profile 证据另行补充。

## 明确非目标

- 不在本焦点实施目标 2 的 image/file/input-audio Native 多模态；
- 不增加真实 Provider/model 注册，不运行真实 Provider、外部 SDK、负载或长期测试；
- 不支持多个 Embeddings Route、跨 Provider fallback、vector identity 等价声明或动态模型发现；
- 不做 streaming、Bridge、向量转换/归一化/降维、缓存、索引、检索、Batch 或 Vector Store；
- 不实现 Images、Files、Uploads、Audio、Videos、Realtime 或其他 OpenAI endpoint；
- 不借本次协议扩展重构无关 Chat/Responses 路径。

## 验证边界

按 TDD 先运行精确失败测试，再实现最小行为并依次运行：

```powershell
cargo test --locked --test embedding_forwarding_contract
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

完成只能证明本地 Rust registry、HTTP JSON、Native forwarding、错误和取消 contract。真实 Provider model、当前外部 SDK、向量质量/等价性、吞吐、费用与生产限制保持未验证；完成后将实际证据写入 implementation status，并把本文件恢复为空焦点。
