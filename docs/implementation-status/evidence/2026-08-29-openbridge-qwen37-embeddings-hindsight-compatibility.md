# 2026-08-29 OpenBridge Qwen3.7 Embeddings 与 Hindsight 兼容性验证

## 范围与环境

- 时间：2026-08-29 17:08–17:48 CST（UTC+08:00）
- 下游 endpoint：`https://llmapi.icebluesakura.xyz/v1`
- Public Model：`qwen3.7-text-embedding`
- 本地源码基线：`f32f774faee4ada340e9e03ed6dc9f8e03268144`
- 本地 worktree：测试期间存在用户既有 `src/providers/catalog/public_models.rs` 修改和本任务未提交修改；未读取或记录 credential 值
- 客户端：Python 标准库独立 HTTP probe；另以 Hindsight 0.9.2 amd64 容器 digest `sha256:3b46e26ec69355422c46ceb496cd758ae226d751be4a0799b4e844251d524d46` 内实际 `openai==2.24.0` 执行合成请求，并核对对应 revision `ebad478240d3171bb88201ececda5e8d9883d22d`
- 输入：仅合成中英文短句，不含真实记忆、用户正文或生产业务数据

本记录描述当时已部署 OpenBridge 的实际观察。未读取远端部署 revision，因此本地 checkout 不能证明线上二进制身份。

## 已执行结果

### Models 与基础 wire

- `GET /v1/models`：HTTP 200，精确出现一个 `qwen3.7-text-embedding`，`owned_by=openbridge`。
- `POST /v1/embeddings`，`input: list[str]`、省略 `encoding_format`/`dimensions`：HTTP 200。
- response：`object=list`、连续 `data[].index`、`model=qwen3.7-text-embedding`、input/total token usage 可解析。
- 默认向量为 1024 维；6 个样本均为有限数，L2 norm 位于 `0.9999999672..1.0000000441`。
- 同一合成输入重复请求逐元素一致，cosine 约为 1。

### dimensions 与 batch

- 显式 `dimensions=1024`：HTTP 200，返回 1024 维，L2 norm 约 1。
- 显式 `dimensions=512`：HTTP 200，返回 512 维，L2 norm 约 1。
- 20 条 string batch：HTTP 200，返回 20 项且 index 完整。
- 21 条 string batch：HTTP 400，`unsupported_model_capability`，`param=input`。

### 合成语义小样本

查询“用户把 Hindsight 记忆服务部署在哪里？”时：

- 相关中文 passage cosine：约 `0.7866`；
- 相关英文 passage cosine：约 `0.7705`；
- 只与网关相关但答案错误的 passage：约 `0.3704`；
- 无关晚餐 passage：约 `0.1101`。

加入 `Qwen/Qwen3-Embedding-0.6B` 模型包自带的 web-search query prefix 后，相关中文/英文分别约为 `0.7761`/`0.7632`，本小样本没有改善。该结果不证明应禁用 instruction；`qwen3.7-text-embedding` 不是该 0.6B checkpoint，且一个 query 不是 benchmark。

### Hindsight 阻断项

- 带 `user`：HTTP 400，`unsupported_model_capability`，`param=user`。Hindsight 的 bank attribution 默认关闭，因此不是默认阻断；Bailian 文档未声明该字段，OpenBridge 保持 fail-closed。
- 带 `encoding_format=base64`：HTTP 400，`unsupported_model_capability`，`param=encoding_format`。
- Hindsight 0.9.2 锁定的 OpenAI SDK 2.24.0 在应用未显式指定 encoding 时，会把 `encoding_format=base64` 加入实际 HTTP body 并在客户端解码。使用上述固定容器执行真实 SDK 请求，线上 OpenBridge 返回 HTTP 400、`unsupported_model_capability`、`param=encoding_format`；因此这是默认启动 dimension probe 和普通 encode 的 P0 兼容阻断。
- Hindsight 默认 `HINDSIGHT_API_EMBEDDINGS_OPENAI_BATCH_SIZE=100`，高于已验证的 20 条上限；配置必须显式设为 20。

## 本地确定性修复验证

本任务只在 `bailian/qwen3-7-text-embedding` 的具体 Upstream API policy 增加显式 wire translation：

1. 下游请求 Base64 时，上游 body 改为 `encoding_format=float`；
2. 有界成功 body 中每个有限数转换为 little-endian IEEE-754 float32 bytes，再编码为标准 Base64；
3. 维度、数值、index 和 model identity 继续由原有 validator 检查；
4. 完整但乱序的 `data[].index` 在提交前排序；缺失、重复或越界仍失败；
5. `user` 不被静默丢弃或伪装成 Bailian 能力。

已执行：

```text
cargo test --locked --test embedding_forwarding_contract
12 passed; 0 failed

cargo clippy --locked -- -D warnings
PASS
```

因当前 Rust linker wrapper 引用已被 Nix GC 的路径，命令使用当前系统 `clang`/`ld.lld` 的临时 `RUSTFLAGS`；未修改系统或项目工具链。

`cargo test --locked --lib` 最终结果为 117 passed / 0 failed。初次运行曾有 18 项被用户既有 `deepseek-v4-pro` 双协议修改阻断；用户随后明确授权修复，补齐 Bailian Responses `UpstreamApi` 后基线恢复。该 DeepSeek 修复有独立 evidence，不属于 embedding wire translation 本身。

修改后的 OpenBridge 另以独立 `127.0.0.1:18080` 运行，并由固定 Hindsight 0.9.2 容器内实际 OpenAI SDK 调用：默认 startup probe 返回 1 个有限的 1024 维向量，20 条显式 512 维 batch 返回连续 0..19 index，21 条继续在本地以 `param=input` 安全拒绝。临时实例已在验证后停止。

## 未证明范围

- 修复尚未部署，线上 Base64 请求仍为 400；
- 尚未通过真实 Hindsight 0.9.2 runtime 执行 retain/recall；
- 未验证上游长期稳定性、费用、限流、并发、长文本、128K 极限或所有允许维度；
- 未验证真实 bank export/re-import、阈值校准或大规模语义质量；
- 未证明 prefix 对 `qwen3.7-text-embedding` 的收益；部署初期应保持 query/passage prefix 为空并用真实 recall corpus 再决定。

## 来源

- Alibaba Cloud Model Studio Embedding：<https://help.aliyun.com/zh/model-studio/embedding>
- Alibaba Cloud 文本向量同步 API：<https://help.aliyun.com/zh/model-studio/text-embedding-synchronous-api>
- Hindsight 0.9.2 配置与源码 revision：<https://github.com/vectorize-io/hindsight/tree/ebad478240d3171bb88201ececda5e8d9883d22d>
- OpenAI Python SDK 2.24.0 Embeddings 实现：<https://github.com/openai/openai-python/blob/v2.24.0/src/openai/resources/embeddings.py>
