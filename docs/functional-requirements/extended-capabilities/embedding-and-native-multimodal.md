# Embeddings 与 Native 媒体扩展导航及共同规则

## 文档边界

本页是扩展能力的导航和共同规则入口，不再混合记录各功能的具体 wire。功能要求按客户端可观察能力拆分：

| 功能          | 唯一需求入口                         | 实施证据入口                                                                                 |
|---------------|--------------------------------------|----------------------------------------------------------------------------------------------|
| Embeddings    | [Embeddings 能力](embeddings.md)     | [Embeddings 实施状态](../../implementation-status/features/embeddings.md)                       |
| Native 图片   | [图片能力](native-image.md)          | [`mimo-v2.5` Native 图片输入](../../implementation-status/features/native-image-input.md)        |
| Native 文件   | [文件能力](native-file.md)           | [当前实现总览](../../implementation-status/current-implementation.md)                           |
| Native 音频   | [音频能力](native-audio.md)          | [当前实现总览](../../implementation-status/current-implementation.md)                           |

功能需求只定义目标行为、失败语义与安全边界；当前 checkout 已经完成什么、运行过哪些检查，只由 `implementation-status/` 记录。
任何功能进入实现前仍须遵守[当前开发焦点](../../implementation-plans/current-focus.md)的一次一个可观察行为约束。

扩展 schema 尚未发布，因此继续使用首版最佳实践：保持 `schema_version: "1"` 并直接同步 DTO、parser、registry、OpenAPI、配置与
测试，不提供旧字段镜像、兼容 alias、双读写、默认回退或无意义版本迁移。

## 1. 能力事实分层

所有扩展功能必须保持以下四层分离：

| 层                               | 拥有的事实                                                                       | 不得替代的事实                                         |
|----------------------------------|----------------------------------------------------------------------------------|--------------------------------------------------------|
| Canonical Model                  | task、输入/输出模态、上下文或向量本体事实                                        | endpoint wire、媒体来源、参数、限制或 Route            |
| Provider/Upstream API            | 受信 endpoint 支持的输入/输出形状、来源、格式、选项和 served limits              | 下游 Public Model 身份或动态请求选择                   |
| Public Model execution interface | 全部静态可执行 Route 的保守交集，以及与该交集绑定的固定候选顺序                  | Provider/Target 拓扑、credential、运行时健康或能力并集 |
| Request requirements             | 本次请求实际使用的 form、role、source、format、数量及可直接计算的资源事实        | 重新筛选、跳过或重排 Route 的依据                      |

Canonical modality 只能证明模型可能消费或产生某类数据，不能自动打开 API 能力。`image_input: true`、`file_input: true`、历史上的
`audio_input`/`audio_output` bool 或笼统 `multimodal: true` 都不足以成为可执行公共契约；当前音频 presence 必须由 typed profile 推导。

## 2. 能力编译与请求预检

- 公共能力由同一个 `ModelExecutionInterface` 的全部静态可执行 Route 保守编译；集合取交集，数值上限取能够保证的最小值，default
  必须一致。
- 未知或缺乏证据的能力按不支持处理，不能因 Native passthrough、首选 Route 较强、Models list 或 canonical modality 提升。
- Bridged Route 只贡献当前 converter 能无损表达的共同子集；本阶段对 image/file/audio source 与 audio output 贡献空集。
- 请求分析冻结对应功能页要求的实际 form、role、source、encoding、format、detail、voice、数量和资源 facts；preflight 只与固定
  interface 比较。
- preflight 通过后 planning 仍使用完整候选顺序，不能根据某个媒体 part、encoding 或输出 mode 临时跳过较弱 Route、改选 Provider
  或求能力并集。
- 标准 Models 接口继续保持四字段；扩展 Models 不暴露 Provider、Target、Route、upstream model、endpoint、credential、内部
  identity 或运行状态。

每个功能的闭合集合和额外编译规则只写在其功能页；跨功能规则不得反向扩大某个具体 interface。

## 3. Native 保真与 Bridge 边界

Native 转发必须保持请求 part/项目顺序、类型、source data、格式/选项、成功响应和原协议 terminal。除受信
model/path/auth/header 改写及 Public Model response projection 外，不得下载并替换媒体、转换 embedding、把媒体转成文本、丢弃字段
或改变编码。

Chat ↔ Responses Bridge 对本阶段媒体请求保持 fail closed。只有未来建立逐字段、逐事件的无损表达证据后，才能在对应功能需求中
开放某一具体转换方向；共享页面不能授权 Bridge。

## 4. 资源与数据保护

- JSON body、单个 content part、累计 inline encoded bytes 和安全解码后的 bytes 必须分别有界；remote URL 另有长度上限。
- URL 只能作为业务内容，不能控制 upstream base URL、Host、Authorization、credential、proxy 或 header transform。
- remote source 只接受有长度上限的 absolute HTTPS URL，拒绝 userinfo、localhost 及显式 loopback/link-local/private/reserved IP
  literal；OpenBridge 不主动下载或解析 redirect。
- Provider-side DNS、redirect、下载时限、远端 MIME/大小与内容安全属于真实 Provider 验收边界，不能由入站语法检查替代。
- inline data 必须在大分配前完成 encoding、media type 与 byte limit 检查；URL query、filename、resource ID、原始媒体、Base64、
  transcript、向量、完整响应和敏感错误上下文不得进入普通日志或 metrics label。
- 请求与 Provider attempt 只能使用稳定低基数 operation/task 属性；不同功能的 token、seconds、media bytes 与向量统计不得混写。

## 5. Retry、取消与证据

- 只有请求 body 未超过 replay budget 且响应尚未提交时才可按对应功能的幂等边界有限重放；超过 replay budget 的合法大请求只执行
  第一次 attempt。
- 下游取消必须停止当前发送/接收和待执行 backoff；首个业务输出提交后不得 retry、fallback 或拼接另一响应。
- 跨 Target fallback 需要功能特有的等价性证明；同名模型、相同 modality 或共用 endpoint 都不是证明。
- canonical fixture、确定性 Rust test、独立客户端/SDK、真实 Provider、负载和长期运行是不同证据层；每个功能页必须列出其验收项，
  未运行层不得声称通过。

## 6. 共同非目标

- 通过请求期 capability routing、动态 Provider discovery 或未知字段 passthrough 扩大固定公共契约；
- 未经功能页明确授权的专用 Images/Files/Uploads/Vector Stores/Videos/Realtime 资源或会话 API；
- 媒体下载代理、格式转换、OCR、通用转写、内容托管、向量检索或通用安全扫描服务；
- Provider-issued resource identity 的跨账户、跨 Target 或跨 Provider 猜测与迁移。

外部协议入口见[OpenAI 细粒度协议调研索引](../../references/openai/README.md)。
