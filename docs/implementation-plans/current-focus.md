# 当前开发焦点

## 状态

**无进行中的行为。** P0 `mimo-v2.5` Chat Native 通用音频理解已完成；实现事实、失败优先证据、验证命令与未覆盖边界已转入
[Native MiMo 音频实施状态](../implementation-status/features/native-mimo-audio.md)。选择下一项前必须重新建立单一、已批准的当前焦点。

## 多模态优先级

下表记录方向顺序，不把 P1 之后的候选提升为已批准产品范围；完成 P0 后仍须为下一项重新建立单一当前焦点。

| 优先级 | 方向 | 本轮状态 | 进入条件 |
|---|---|---|---|
| P0 | `mimo-v2.5` Chat Native 通用音频理解 | 已完成 | Chat-only 单 WAV data URL profile、JSON/SSE、Models 投影与 zero-egress 负向矩阵均已验证 |
| P1 | `POST /v1/audio/speech` | 未批准、延期 | 先完成独立 F1 binary transport fake contract，并确认 Native Provider 或显式转换边界 |
| P2 | `POST /v1/audio/transcriptions`；非流式 `POST /v1/images/generations` | 未批准、延期 | 前者先具备 multipart/response-union contract；后者先完成真实 Provider operation 调研与探测 |
| P3 | Native file input | 未批准、延期 | 先确认可执行 Provider/model、source profile 与 resource-affinity 边界 |
| 延期 | Responses audio、Realtime、video | 非当前方向 | 需要独立协议、session/lifecycle 或时效证据，不与 P0 合并 |

## 当前焦点

无。P1 及之后各项仍是未批准候选；进入实现前必须补齐可观察行为、需求、先失败测试、明确非目标与独立验证边界。
