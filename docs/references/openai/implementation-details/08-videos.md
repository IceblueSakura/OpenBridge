# Videos 协议实现细节

**目标状态：** 仅作协议参考，不在现阶段 1/2 实施范围。

## 范围与异步资源模型

Videos API 是长任务资源族，不是返回视频的同步 POST。官方 guide 的基本流程是：

1. `POST /v1/videos` 创建 job，返回 ID 与 `queued`/`in_progress` 等初始状态；
2. `GET /v1/videos/{video_id}` 轮询直到 `completed` 或 `failed`，也可由 webhook 观察终态；
3. `GET /v1/videos/{video_id}/content` 在完成后流式下载 MP4、thumbnail 或 spritesheet；
4. 资源族还可能包含 list/delete、edit、extend、remix 和 reusable character。

当前官方 guide 展示 JSON 与 multipart reference input，而 endpoint OpenAPI 快照对 create 的主 schema 以 JSON 表达。实施时必须按目标 Provider/profile 再次确认每个 operation 的 media type，不能把 guide 示例、Batch JSON 和直接 API multipart 混成一个请求 schema。

资料：[Video generation](https://developers.openai.com/api/docs/guides/video-generation) 与 [Videos API](https://developers.openai.com/api/reference/resources/videos)。

## operation 与 resource affinity

Videos 至少需要独立表达 create/list/retrieve/delete/content/edit/extend/remix/character operations。`video_id`、`character_id`、input File ID 与 webhook event 都绑定 issuer、owner 和 credential scope。

创建后必须向下游返回 gateway opaque resource ID 或维护可验证 issuer 映射；后续 poll/download/edit 不能按 Public Model route 顺序逐 Target 猜测。若源 video、character 和 input file 来自不同 issuer，除非目标 API 明确支持跨域引用，否则在 egress 前拒绝。

Public Model capability 至少覆盖 operation、model、size、duration、input reference forms、output variants 和状态集合。模型支持 text/image/video/audio 等 modality 不等于 Videos endpoint operation 全部可用。

## create、poll 与 download

- create 是计费且非确定的副作用。连接超时不证明 job 未创建；没有真实 idempotency contract 时不自动跨 Target replay。
- poll 是同 issuer 读取，可在下游 body 提交前有限 retry，但应尊重 rate limit 和 bounded backoff。
- webhook 若被代理，需要签名验证、事件去重、issuer/owner 关联和公网 ingress；它不是当前 loopback HTTP listener 的自然延伸。
- content 是 raw binary stream，需验证 status 已完成、variant、Content-Type/Length/Disposition/Range、redirect 和总 byte limit。
- 下载 URL/内容可能短时有效；网关不承诺长期存储，也不把签名 URL 写入日志。

## 生命周期与 fallback

job state 必须保留 `queued`、`in_progress`、`completed`、`failed` 等明确状态及 error/progress 的安全子集。网关不能在一个 Provider job 失败后自动创建另一个 Provider job并沿用同一 ID，也不能把两次生成结果视作等价 retry。

edit/extend/remix 和 delete 都是 target-bound mutation。首个 create 成功返回 ID 后，整个后续生命周期固定 issuer；健康/cooldown 只影响新无状态请求，不能迁移现有 job。

## 安全与资源限制

- prompt、reference media、character/video/file IDs、signed URLs 与视频 bytes 不写普通日志。
- JSON/data URL 和 multipart 分别设置编码/解码/part/总字节限制；下载另有最大 bytes、duration 与 timeout。
- 业务输入不得选择 upstream base URL、webhook destination、credential 或 arbitrary header。
- 若未来暴露 webhook，必须单独建模外网信任、签名 secret、replay protection 与用户隔离。
- 内容政策/版权/人物授权是产品合规边界；HTTP 转发成功不代表这些边界已解决。

## TDD 与验收矩阵

| 层 | case |
|---|---|
| create | JSON/multipart profile、model rewrite、resource ID 包装、timeout unknown outcome、不盲目 replay |
| lifecycle | queued → in_progress → completed/failed，非法倒退、重复 terminal 与 polling backoff |
| affinity | poll/edit/delete/content 固定 issuer；伪造/跨用户/跨 issuer reference 拒绝 |
| content | MP4/thumbnail/spritesheet bytes、headers、redirect/range、EOF、limit 与取消 |
| list/delete | cursor/order namespace、删除副作用与安全 not-found |
| webhook（若实现） | signature、dedupe、event ordering、unknown ID 和 replay |

## 非目标

- 不把 Videos 当作 Images 或 Chat/Responses 的 output modality 开关；
- 不本地轮询到完成后才返回同步 POST，除非另有明确异步封装契约；
- 不转码、托管、剪辑、合并或持久化视频；
- 不跨 Provider 迁移 job、character 或 source resource；
- 现阶段 1/2 实施范围不包含本协议。
