# Zhipu AI China 接入进度与边界

注册与能力事实见 `src/providers/zhipu_cn/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- GLM-5.3、GLM-5.2 与 GLM-5.3-Flash 注册 Chat Native JSON Object；Chat function tools 只公开 Auto choice，不公开请求级
  parallel 或 strict schema。
- 只有官方明确列出的 GLM-5.3 注册 `/api/v1/responses` text-only Native，并保留 Chat bridge 作为更高能力请求的回退。
- 2026-08-31 的有界 JSON/SSE Responses probe 均成功；Responses reasoning、structured output、工具、state，以及文件、视频、
  外部 SDK/Agent、其他账号/区域、负载与长期运行未证明。

## 验证与证据

- 2026-08-31 有界管理员 probe 覆盖 GLM-5.3 Responses JSON/SSE。
- 2026-09-02 双协议矩阵实测（[evidence](../evidence/2026-09-02-dual-protocol-capability-matrix.md)）：
  GLM-5.3-Flash 的 reasoning effort 实测仅接受 `low/high/max`（`none/minimal/medium/xhigh` 被拒绝），
  Responses `reasoning-summary` 与 `include-encrypted-content` 被拒绝而 `prompt-cache-key` 被接受；
  tool-none/named/strict 接受但未命中固定 oracle。该 effort 子集收窄未在注册中体现，待独立获准变更复核。
- 官方模型事实来源见 [references/providers/zhipu-api.md](../../references/providers/zhipu-api.md)。

## 代码 owner

`src/providers/zhipu_cn/`。
