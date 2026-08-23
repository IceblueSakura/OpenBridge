# 阶段 5：Images response lifecycle 与 telemetry 证明

> **状态：候选实施计划，不构成实施授权。** 只有阶段 4 建立唯一 Images attempt 后才能提升。本阶段不处理 capability algebra 或 legacy 删除。

## 1. 可观察结果

成功 Images response 只有在 headers、完整有界 body、JSON shape、output count、format 与 Public Model projection 全部验证后才 commit。body 超限、读取失败、提前 EOF、损坏 JSON、下游取消及 commit 边界都有 operation-specific 证据；普通 telemetry 只记录固定 operation/outcome 与图片数量/尺寸，不包含 prompt、URL、Base64、upstream body 或 credential。

## 2. 已验证基线与 owner

- `src/ingress/forwarding/image_response.rs::validated_images_response` 已在 commit 前用 `to_bytes(..., max_body_bytes)` 读取、验证并投影完整 body。
- `src/ingress/forwarding/images.rs` 当前把所有 response validation error归为 502 `invalid_upstream_response`，但 body error、EOF、limit、cancel 与 attempt terminal 的专项证明不完整。
- `RequestObservation::record_images_usage` 已拥有低基数图片 usage；阶段 4 将提供唯一 active attempt。
- `tests/images_forwarding_contract.rs` 当前覆盖正常 wire和部分 preflight，不足以外推完整 body lifecycle。

## 3. RED

1. body 超过 `max_json_response_body_bytes`：commit 前 502、attempt stream/body failure、无 image内容泄漏。
2. body source 返回 typed error或提前结束为损坏 JSON：commit 前稳定 502，attempt/request各唯一终态。
3. valid JSON 但 output count、format、model projection 不匹配：稳定 502，零 partial downstream body。
4. 下游在响应准备/发送边界取消：upstream body future释放，不重复 terminal；若完整 body 已验证，取消只归 downstream delivery。
5. 成功只记录 validated image count/width/height；prompt、URL、`b64_json`、upstream error body和 request body不进入 trace、metrics、stdout或普通 snapshot。
6. non-success upstream body继续由阶段 4 HTTP owner归一化，不因读取诊断 body泄漏内容。

## 4. 实施步骤

1. 为 Images response read建立闭合结果分类：too large、body transport、invalid JSON/shape、contract mismatch、cancel、success；不解析错误字符串。
2. 让 `validated_images_response` 在每个失败分支完成阶段 4 的 active attempt，并向 request observation记录一个稳定 error type/failure stage。
3. 保持全量 precommit buffering有界；禁止 streaming图片JSON或在验证前创建 downstream success body。
4. 明确 cancellation owner：读取中取消释放 upstream；验证后 downstream drop不回写 Provider failure。
5. 只在完整 validated success 后调用 `record_images_usage` 和 attempt complete；usage数值从验证对象产生，不从未验证 JSON提取。
6. 扩展 OTLP/JSONL安全测试，检查 prompt、URL、Base64 marker、header token和upstream error body均不出现在普通 telemetry。
7. 更新 Images requirements/status和未证明边界；不把 synthetic body tests描述为真实 Provider SLA。

## 5. 非目标

- 不增加 Images retry/fallback或改变阶段 4 timeout mapping。
- 不修改 request capability、profile intersection、Models projection schema或Provider注册。
- 不记录单张图片URL、大小分布以外的内容或用户字段。
- 不实现 binary/streaming Images response扩展。

## 6. 验证

Focused：

```text
cargo test --locked --test images_forwarding_contract
cargo test --locked --test observability_contract
cargo test --locked --test otlp_trace_contract
cargo test --locked --test otlp_metrics_contract
```

随后执行完整 Rust baseline、内容安全扫描、Markdown/OpenAPI links 与 `git diff --check`。真实 Provider、负载、内存峰值和长期取消行为另行验收。

## 7. 退出与回滚

完成门：所有 body终态均在 commit 前稳定映射；attempt/request唯一；cancel owner明确；validated success才记录image usage；无敏感内容；current focus清空。回滚必须同时恢复 body owner、observation、tests与状态文档。
