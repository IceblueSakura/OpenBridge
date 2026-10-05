# 当前开发焦点

## 当前范围

下一片为 **`POST /v1/images/generations` 的静态多图生成**：把单图结果迁移为有序产物集合，贯通标准请求、独立 IR、目标映射、严格完整交付与固定 SDK 消费。不是只放开 `n`，也不通过拆成多个上游请求模拟多图。图片方向仍按[计划](next-goal.md#推进顺序与退出条件)优先完善 generation；本片闭合后再定稿流式预览/最终产物事件，API 较完善后再追加其他模型验证。文件扩展此次维持延期，下一次选片时重评，不以所有图片能力完成为前置。

独立 Images 的具名计量损失只按[投影合同](../architecture-v2/protocol-and-lowering.md#独立-images-的计量投影)适用，不授权丢弃请求控制、篡改产物报告或隐藏预算失败。新付费测试需要明确场景，不复用旧矩阵授权扩大控制或模型范围。

文件仅维持既有 Responses user inline/URL 基础输入及必要正确性、安全维护；其他文件功能暂停，恢复评估按[计划的有限首批范围与重评节点](next-goal.md#推进顺序与退出条件)执行，不等待所有模态完成，也不自动恢复实施。不新增 `/v1/files` 上传、存储、下载、删除或 file_id 服务。Inline 与 URL 文件合同分别归 [inline profile](../architecture-v2/responses-text-profile.md#user-inline-file-input)和 [URL profile](../architecture-v2/responses-text-profile.md#user-file-url-input)，其他模态的下一片按[计划](next-goal.md)单独定稿；文件扩展延期，不能从 PDF carrier 推定所有格式、来源或工具文件均准入。

文件与必要 opaque 回传的验证应分别覆盖“实际报告且回传”和“未报告”；后者即便内容正确也不证明 opaque 路径。显式 reasoning 控制不充当已生成 reasoning 的事实，有限场景通过不等于一般可靠性。

## 静态多图切片

### 可观察结果与标准依据

- 标准客户端可提交有界的多图计数并消费有序 `data` 数组；缺省/null 的单图行为保持，图片顺序、内容和实际报告不得合并、重排或补造。
- 实施前按[固定来源](../references/upstream-sync.md)、[OpenAI 来源](../references/openai/README.md)及[Provider 来源](../references/providers/README.md)核对现用标准/profile：`n` 的含义、各目标范围、少图与空结果的合法状态、报告属性的作用范围。不能把请求数量当作实际生成数量，也不能无依据地把所有目标统一为“必须恰好返回 n 张”。来源不能解决的数量/完成语义分歧先报告并确认，不猜测。
- 以标准与当前目标的交集确定公开准入，目标可进一步收紧。仅扩展现有 generation 接线；静态注册、实例激活、上游接受与内容效果仍分层验证。

### IR 结构与迁移

当前 [ImageGenerationResponse](../../src/semantic/task/image_generation.rs) 只有单个 `image`。采用 **同一 ImageGeneration task 下的有序 GeneratedImage 集合**，而非复用 conversation items、为每张图创建一次执行，或在 adapter 中藏第二套结果结构。集合顺序有语义，但不据此发明跨响应产物 ID。

先明确请求计数、实际产物集合、响应闭合和请求满足程度的关系，再确定 Rust 布局。每张图保留自己的 bytes 与可用报告；响应级计量仍归整次生成，不按请求数量分摊、复制或补算。共享 wire 报告的作用范围须由来源确认：只有确实适用于集合的值才能投影成标准顶层属性；不能拿首图代表其余图片，不能为输出方便丢弃异构报告。无法表示且不在既有计量损失白名单内的内容应拒绝。

同步迁移构造器、验证、标准/OpenRouter codecs、lowering、Gateway、独立消费者、OpenAPI、fixtures 与受影响 probe。允许移除旧单图 Rust 字段，不保留互相竞争的单图/多图权威或兼容垫片；`n=1` 的既有 wire 行为与安全边界要有回归保护。

### 不变量、预算与失败例

- 每个固定候选从同一不可变输入出发，最终 typed 值决定编码与要求；删除、替换或重排产物后必须重新校验，不能恢复旧值或遗留索引/报告。计量损失策略不得掩盖产物或数量合同失败。
- 分别界定请求计数、实际产物数、单图 decoded bytes、集合累计 decoded bytes、完整 JSON 与 operator/endpoint 上限；在分配/收集前后各自控制预算。沿用现有有限边界，明确新增集合预算的 owner、默认值和硬上限，不按 `n` 自动放大内存或绕过更紧限制，不扩大对话 JSON 预算。
- 一次上游请求、一次静态完整结果；合法结果必须到达严格 EOF 并全部验证后才能发布。后续某张图损坏、截断、超预算或报告冲突，不能返回已验证的前缀数组冒充整次成功；不增加 retry、fallback、自动补图或后台续跑。
- 独立反例至少包括：非法/目标不支持的计数，违反已定稿合同的空、少图或超额结果，后项坏 Base64，逐图与累计预算边界，混合格式/尺寸/背景等报告的可表示性，明确请求与实际报告冲突，尾部垃圾/截断、取消和超时。若合同允许少图，必须另有合法少图预期，不能只测拒绝路径。

### 验证与退出条件

1. **先 TDD、最低 owner 验证**：独立 wire→IR 与 IR→wire 预期，类型化构造/编辑/删除/重排，数量关系、计量归属与资源边界；不以 round trip 作为唯一 oracle，不让纯 codec 测试依赖产品 catalog。
2. **交付与消费**：在既有 synthetic HTTP/Router 与固定 SDK gate 中覆盖多图有序消费、保持单图兼容及整包失败，不为同一事实堆叠重复 Gateway。probe 的旧三场景和单图预算不能自动消费多图计划；若扩展工具，须同步明确图片数量预算、实际发送守卫和每项 oracle，否则显式维持单图拒绝，不发 live 请求。
3. **基线与文档**：按[开发指南](../development.md)运行受影响检查、Rust 基线与适用的显式 SDK gate；同步 OpenAPI 的数量、报告和预算约束，检查示例/引用。补齐完整 Schema validator 检查；缺少依赖时说明验证缺口，不以仅解析 JSON 冒充 Schema 校验。
4. **退出条件**：标准静态多图从请求到 typed 消费贯通，合法少图与失败边界符合已确认合同，单图与既有文本/文件行为不回退，默认测试不读取真实凭据或调用 Provider。审阅最终 diff，在对话报告结果和未验证层，清除本片焦点但保留后续方向。

### 非目标与执行边界

不实现流式、partial images、图片编辑/蒙版/参考图、URL 下载或文件服务，不新增模型、Provider、私有客户端字段或通用产物框架，不扩展 Chat/Responses 图片生成 carrier，不做生产负载与质量评估。真实多图调用、其他模型验证、凭据操作、部署和提交仍需相应明确授权；旧单图矩阵和预算偏好不授权新请求矩阵。当前这次会话仅写本文件，代码实现与执行验证留待新会话。

## 必要 replay 合同

已闭合 item 的完整 opaque 值若与终态不同，现行拒绝继续生效；不得按仅终态新增的方案处理已有值替换。该缺口保留但不作为下一片或其他模态的前置；重新选片时先按[实现缺口](../implementation-status/generation.md#语义与表示缺口)定清 replay 值/凭据身份、最终 authority 与更新的依赖影响，再决定最小事件和具名 wire 规则。未定稿前不增加 replay 事件、放宽 snapshot 或补建通用 attachment/framework。

独立 `_openbridge` 不属于[当前客户端合同](../architecture-v2/client-generation-profile.md)，迁移不恢复隐式兼容入口。允许破坏性重写，但新的 IR 结构缺口仍应报告概念方案和迁移影响；Chat 有损规则逐条定稿，不以方向许可提前丢字段。

实施按上面的可观察结果、需求、不变量、失败例、非目标与验证边界执行；出现未覆盖的结构或标准分歧时先更新/确认切片，不以计划代替操作授权。实现缺口归[状态文档](../implementation-status/generation.md)。
