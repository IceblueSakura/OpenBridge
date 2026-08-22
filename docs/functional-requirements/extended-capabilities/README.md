# Embeddings 与 Native 媒体扩展

本页是扩展能力的导航和共同规则唯一入口。各功能的 wire、限制与验收项只写在对应功能页；实现与验证事实
统一见[实施现状](../../implementation-status/README.md)。

| 功能 | 唯一需求入口 |
|---|---|
| Embeddings | [Embeddings 能力](embeddings.md) |
| Native 图片 | [图片能力](native-image.md) |
| Native 文件 | [文件能力](native-file.md) |
| Native 音频 | [音频能力](native-audio.md) |
| Images 生成 | [Images 生成](native-image-generation.md) |

## 1. 能力事实分层

| 层 | 拥有的事实 | 不得替代的事实 |
|---|---|---|
| Canonical Model | task、输入/输出模态、上下文或向量本体事实 | endpoint wire、媒体来源、参数、限制或 Route |
| Provider/Upstream API | 受信 endpoint 的输入/输出形状、来源、格式、选项与 served limits | 下游 Public Model 身份或动态请求选择 |
| Public Model execution interface | 全部静态可执行 Route 的保守交集及其固定候选顺序 | Provider/Target 拓扑、credential、健康或能力并集 |
| Request requirements | 本次请求实际 form、role、source、format、数量及可直接计算的资源事实 | 重新筛选、跳过或重排 Route 的依据 |

Canonical modality 只能证明模型可能消费或产生某类数据，不能自动打开 API 能力。粗粒度
`image_input`/`file_input`/`audio_input`/`audio_output` bool 或 `multimodal: true` 都不能替代完整的可执行
profile。

## 2. 能力编译与请求预检

- 公共能力由同一 `ModelExecutionInterface` 的全部静态可执行 Route 保守编译：集合取交集，数值上限取
  可保证的最小值，default 必须一致。
- 未知或没有完整 contract 的能力按不支持处理，不能因 Native passthrough、首选 Route 较强、Models list
  或 canonical modality 提升。
- Bridged Route 只贡献 converter 能完整表达的共同子集；image/file/audio source 与 audio output 不贡献
  Bridge 能力。
- 请求分析冻结对应功能页规定的 form、role、source、encoding、format、detail、voice、数量与资源 facts；
  preflight 只与固定 interface 比较。
- preflight 通过后仍使用完整固定候选顺序，不能根据媒体 part、encoding 或输出 mode 跳过较弱 Route、
  改选 Provider 或求能力并集。
- 标准 Models 保持四字段；扩展 Models 不暴露 Provider、Target、Route、upstream model、endpoint、
  credential、内部 identity 或运行状态。

## 3. Native 保真与 Bridge

Native 转发必须保持请求 part/item 顺序、类型、source data、格式/选项、成功响应和原协议 terminal。除受信
model/path/auth/header 改写及 Public Model response projection 外，不得下载并替换媒体、转换 embedding、把媒体
转成文本、丢弃字段或改变编码。

Chat-to-Responses 与 Responses-to-Chat Bridge 对媒体请求保持 fail closed；只有对应功能需求定义了逐字段、逐事件
的完整转换契约后，才可开放某个具体方向。

## 4. 资源与数据保护

- JSON body、单个 content part、累计 inline encoded bytes 与安全解码后的 bytes 必须分别有界；remote URL
  另有长度上限。
- URL 只是业务内容，不能控制 upstream base URL、Host、Authorization、credential、proxy 或 header transform。
- remote source 只接受有界 absolute HTTPS URL，拒绝 userinfo、localhost 及显式 loopback/link-local/private/
  reserved IP literal；OpenBridge 不主动下载，也不解析 redirect。
- Provider-side DNS、redirect、下载时限、远端 MIME/大小与内容安全不由入站语法检查替代。
- inline data 必须在大分配前检查 encoding、media type 与 byte limit。
- URL query、filename、resource ID、原始媒体、Base64、transcript、向量、完整响应和敏感错误上下文不得进入
  普通日志、trace attribute 或 metric label。
- 不同功能的 token、seconds、media bytes 与 vector 统计不得混写。

## 5. Retry 与取消

- 只有 request body 未超过 replay budget 且 response 尚未提交时，才能按对应功能的幂等边界有限重放；
  超过 replay budget 的合法请求只执行第一次 attempt。
- 下游取消必须停止当前发送/接收与待执行 backoff；首个业务输出提交后不得 retry、fallback 或拼接另一响应。
- 跨 Target fallback 需要功能特有的等价 identity；同名模型、相同 modality 或共用 endpoint 都不是证明。

## 6. 共同非目标

- 请求期 capability routing、动态 Provider discovery 或未知字段 passthrough；
- 未经功能页明确授权的 Images edit/variation、Files/Uploads/Vector Stores/Videos/Realtime 资源或会话 API；
- 媒体下载代理、格式转换、OCR、通用转写、内容托管、向量检索或通用安全扫描；
- Provider-issued resource identity 的跨账户、跨 Target 或跨 Provider 猜测与迁移。

外部协议事实见[OpenAI 细粒度协议调研](../../references/openai/README.md)。
