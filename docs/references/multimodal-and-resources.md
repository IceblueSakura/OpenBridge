# 多模态与资源语义基线

来源：2026-09-23 Responses Create 与 SDK 固定类型；历史 image/file/audio、特殊语音 wire 研究已整合于本页，精确版本与链接见[同步基线](upstream-sync.md)和[历史来源](semantic-baseline.md#8-历史来源追溯)。本次没有下载媒体、调用模型或验证 codec/playback。新模态、格式、source 类型或 operation 变化时重核。

## 1. 标准优先，缺少标准的能力才扩展

| 语义域 | 当前公开依据 | IR 方向 |
|---|---|---|
| Responses 文本输入 | input_text、cache breakpoint | 标准 content part |
| Responses 图片输入 | URL/data URL/file ID；detail 含 auto/low/high/original | 标准 image part，source 与 detail 不压成 text |
| Responses 文件输入 | file_data/file_url/file_id、filename、detail、cache breakpoint | 标准 file part；source 与处理用途分离 |
| 工具多模态结果 | function/custom result 内容数组，computer screenshot、hosted image output 等各自 schema | 按对应 result/item 建模，不统一 stringify |
| Responses 音频 | 事件目录存在 audio/transcript delta/done；本次 SDK 输入 content union 仍为 text/image/file | 已声明事件是标准证据，但不能据此猜出完整音频 create→output 契约；需单独补齐来源与 profile |
| Provider 特殊音频/视频 | 历史资料展示 Chat-shaped ASR/TTS 等并非标准 Audio endpoint | 经固定操作合同进入 task-specific typed extension，不伪装成普通 conversation |

标准图片/文件不能因当前实现没有 codec 就降格为“Provider 特殊能力”。同样，存在 audio 事件也不等于任意 Responses model 支持 audio input/output。

## 2. Resource value 需要表达什么

资源身份、来源、格式和用途分别表达：

- 来源：inline bytes / encoded inline data、remote URL、issuer-bound file/resource ID；source 的组合合法性按具体 part schema 验证。
- 描述：media type、编码/format、filename，必要的尺寸、采样率、channel 等只在有协议依据时加入。
- 用途：user perception、tool result、reasoning artifact、生成媒体或 voice reference；共享 source 不能抹掉用途。
- 选项：image/file detail、音频任务控制、mask、局部时间信息和 streaming chunk 关系。
- 依赖：稳定 item/part/resource identity、source issuer、access/retention 和跨目标 replay 约束。

Provider 返回的 file ID、音频 ID、container ID 和签名 URL 不是可移植正文；换 profile 不能猜测替换或自动重新上传。源 annotations、cache breakpoint、signature 随 owner 生存，不能按数组下标重新附着。

## 3. 不由纯 codec 承担的工作

下载、DNS/redirect、MIME sniffing、音视频转码、上传、扫描、缓存和资源授权属于显式资源/执行服务。纯 codec 不因收到 URL 就发网络请求，也不以 transcript 替代音频并声称无损。

encoded 与 decoded bytes、单资源与总请求、解压/解析深度、增量 chunk、capture 与 cancellation 分别有界。Base64、文件名、URL query、原始媒体及 transcript 都可能敏感，不能把它们自动放入日志或测试资产。

## 4. Task / modality / wire 不混同

- Responses hosted image generation 属于 Generation 的 tool 生命周期；独立 Images generation/edit 是独立 task/operation。
- 标准 Audio transcription/speech 与 Realtime session 各有独立 contract。
- 特殊 Provider 用 Chat envelope 提供语音时，wire 名称不决定任务类型。
- Responses-first 约束 Generation 的主模型，不要求把 Embedding、VoiceDesign 等塞进 Responses 的 message union。

## 5. 后续验收的最小单元

先选一个明确 source+task+profile，建立独立 request/response/event 预期，再覆盖：

1. source 分支与互斥/缺失组合、detail/presence/filename；
2. IR 插入、替换、重排、删除后资源依赖不串位；
3. 资源 reference 同来源接受、异来源拒绝；
4. bytes、encoded/decoded 预算、分片、截断与取消；
5. 静态结果与增量 materialization 一致。

小型 synthetic 媒体可证明表示和资源边界，不证明 OCR、音质、语音授权、模型质量或 Provider 下载行为。历史 MiMo 观察只适用于当时 endpoint 与样本，不上升为通用多模态规则。
