# Files 协议实现细节

**目标状态：** 仅作协议参考；已从现阶段实施目标移除。

## 范围与完整资源面

Files 是资源生命周期，不是单一上传 endpoint：

| operation | OpenAI path | wire 形状 |
|---|---|---|
| create | `POST /v1/files` | `multipart/form-data` file + purpose；返回 File JSON |
| list | `GET /v1/files` | query；返回分页/list JSON |
| retrieve | `GET /v1/files/{file_id}` | path resource ID；返回 File JSON |
| delete | `DELETE /v1/files/{file_id}` | 有副作用；返回 deletion JSON |
| content | `GET /v1/files/{file_id}/content` | 返回原始文件 bytes 与媒体 headers |

“OpenAI-compatible Files”若只实现 create 而不说明 list/retrieve/delete/content 边界，会产生无法管理的资源。对外契约应逐 operation 声明支持状态；不支持的 operation 在本地稳定拒绝，而不是转发到猜测的路径。

官方资料：[Files API](https://developers.openai.com/api/reference/resources/files)、[Create file](https://developers.openai.com/api/reference/resources/files/methods/create) 与 [File inputs](https://developers.openai.com/api/docs/guides/file-inputs)。当前 OpenAI 服务限制和 purpose 枚举会变化，应作为 Provider profile 事实复核，不作为网关全局常量。

## 资源身份与路由

`file_id` 由特定 Provider、项目/credential scope 和 Target 签发。下游只看见裸 upstream ID 时，后续 retrieve/delete/content 无法从 ID 安全推断 issuer。实现需要在两种策略中明确选择：

| 策略 | 行为 | 代价 |
|---|---|---|
| gateway opaque ID | create 后生成本地 ID，ledger 保存 upstream ID + issuer + owner | 需要持久化、授权、重启恢复、删除与迁移规则 |
| issuer-encoded opaque ID | 签名封装 issuer 和 upstream ID，不保存映射 | 需要密钥轮换、长度/版本、不可伪造与 credential scope 设计 |

不能依赖 upstream ID 前缀或逐 Target 试探。未知、篡改或不属于当前用户的 resource ID 必须在 egress 前返回安全 not-found/invalid-resource 结果，不暴露候选列表。

list 必须定义聚合语义。简单拼接多个 Provider 的列表会破坏分页 cursor、排序、purpose filter 和稳定性；第一版更安全的选择是一个 Files namespace 固定绑定单一资源 issuer，或由显式 gateway ledger 统一分页。

## multipart create

上传 handler 必须支持有界 multipart，并选择原始流式转发或解析重建。无论哪种方式，都需要：

- 唯一且带 boundary 的 `Content-Type`；
- file/purpose 等字段 allowlist、重复字段策略和 part count；
- 编码/实际总字节、单 part、字段名和值长度限制；
- filename 规范化，不信任路径分隔符、控制字符或 MIME；
- 下游取消和上游 backpressure；
- 若使用临时文件，受限目录、权限、配额与所有退出路径清理；
- 真实 upstream model/endpoint/auth 不受 multipart 字段控制。

purpose 是资源语义，不只是标签。它会影响文件类型、大小、后续可用 endpoint 与保留策略；必须按 Provider profile 预检，不能静默替换。

## retrieve/delete/content

- retrieve 按 gateway resource ownership 定位唯一 issuer，保留安全元数据字段，不暴露 Provider/Target/credential。
- delete 是不可逆副作用；网络超时后结果可能未知，不能跨 Target fallback 或无界自动 replay。
- content 是 raw binary response，不能经过 JSON/SSE parser。保留 allowlist `Content-Type`、长度、disposition/range 语义前必须逐项验证。
- list/retrieve/content 可以在未提交下游 body前对同一 issuer 做有限读取 retry；resource 操作不得切换 issuer。
- 若上游返回 redirect，网关必须有明确 follow/relay policy，不能把带签名的内部 URL写入日志。

## 用户隔离与安全

即使 OpenBridge 当前是单配置所有者、少量受信用户，Files 仍需要 owner binding。一个已认证用户不能仅凭猜到的 ID读取或删除另一个用户创建的资源。

不记录文件内容、filename、purpose 之外的敏感 metadata、完整 ID 或下载 URL。错误信息不得回显本地临时路径、upstream path、项目 ID 或 credential scope。默认不缓存、扫描、解析或持久化文件正文；若产品需要病毒扫描/DLP，应作为独立行为与证据边界。

## TDD 与验收矩阵

| 层 | 必须证明 |
|---|---|
| create | multipart boundary、字段/part/byte limits、purpose、filename、取消、model-free trusted routing |
| identity | opaque ID 不可伪造；issuer/owner 唯一；未知 ID 不发出跨 Target probe |
| list | pagination/filter/order 的明确 namespace 语义，不重复或泄露其他用户资源 |
| retrieve/delete | 同 issuer 路由；delete timeout/重复调用边界；安全 not-found |
| content | raw bytes、safe headers、EOF/range/redirect policy、byte limit 与取消 |
| restart | 若有 ledger，重启恢复、一致性、过期和删除后的映射行为 |

真实 Provider 测试还要验证 purpose、文件类型/大小、项目配额、删除可见性和内容下载。mock/corpus 不证明云端资源权限或持久性。

## 非目标

- 本文不把 Uploads、Vector Stores、Batch、Fine-tuning 或 File Search 自动纳入 Files；
- 不通过逐 Provider 试探解析裸 `file_id`；
- 不做通用对象存储、CDN、文件转换、OCR、DLP 或杀毒；
- 不在没有 durable identity/ownership 方案时宣称完整 Files 兼容；
- 现阶段 1/2 实施范围不包含本协议。
