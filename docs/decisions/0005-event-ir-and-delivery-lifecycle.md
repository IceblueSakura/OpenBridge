# ADR-0005：Event IR 权威、静态一致性与交付生命周期

## 状态

- **决策：承接已接受的增量 IR 与生命周期边界。** 本页具体化 [ADR-0001](0001-generation-ir-authority.md) 的流式原则和 [ADR-0002](0002-task-ir-and-semantic-ownership.md) 的 Static/Event 一致性约束。
- **实现：Native Event 所有权仍在迁移。** 具体路径和缺口见[架构](../architecture.md#62-generation)与[实施状态](../implementation-status/current-boundaries.md#generation-与-bridge与-ir-权威目标的差距)。已通过 reducer 不等于输出已完全由 Event IR 决定。
- 对外失败、retry/fallback、timeout 和 commit 的详细规则仍由 [Native 与流式合同](../functional-requirements/gateway-api/native-and-streaming.md)及[路由合同](../functional-requirements/routing-resilience.md)拥有；本页不另造错误规则，不独立授权代码变更或工具拦截。

## 背景与问题

流式路径可以 decode 并校验 canonical events，却仍从原始 event payload 生成下游正文。这样的结构能保护生命周期，但不能让未来受信 IR 变换约束真实增量输出。另一方面，强制聚合整条流以复用静态 encoder，会改变延迟、预算、背压与取消边界。

因此必须同时定义 Event 的语义权威、与 Static IR 的对应关系，以及语义处理不能越过的交付边界。

## 决策

### 1. 增量语义权威

SSE 经有界 framing、按任务/profile decode、Event IR 校验及 reducer，再按下游目标 lowering/encode；总阶段顺序见 [ADR-0003](0003-ir-pipeline-and-target-compilation.md)。原始 event 仅按 [ADR-0004](0004-source-records-and-fidelity.md) 提供合法的有界表现提示或扩展，不能覆盖 IR 决定的正文、身份与终态。

纯 codec/reducer 不拥有 routing、retry、credential、网络 I/O 或 downstream commit。未来获准的受信处理只能影响未提交内容，且必须维护验证状态与编码状态的一致性；不能只改变 reducer 里的值，却继续输出旧 payload。需要等待完整工具调用的拦截须另定有界缓冲和提交契约，不由本页开放工具运行时。

### 2. Static 与 Event 的语义一致性

对同一任务、固定 profile 和明确对应的静态/事件样本，合法结束的 Event state materialize 后，应与独立静态语义预期一致。正文、工具身份及参数、refusal、reasoning、usage、资源关联与任务终态不因 delivery 模式改变 owner。

一致性不要求网络分片、事件数量、JSON key order 或 bytes 相同，不要求不同真实模型调用生成相同结果。并非每个中间 Event prefix 都可 materialize；只有任务合同允许的合法结束状态才能形成对应静态响应。failed/incomplete/cancelled 不得为得到静态结果而改成 completed。

### 3. 语义终态与传输结束分离

item.done 不等于整个 response 完成，有字节到达不等于成功，EOF 不创造任务终态。合法空输出、refusal、部分结果、失败和取消必须保留区别；实际 Provider 终态与 body error 不得相互伪装。

稀疏 completed terminal 只能用此前已经验证的内容进行确定性补齐。缺少真实 terminal 时，不得合成 completed、failed 或 `[DONE]` 让客户端正常收尾。合法终态后的普通 close 不反转已确认结果；其余非法后续数据继续按协议和 body 合同处理。

### 4. 沿用既有 commit 边界

上游成功 headers 不代表下游已经提交。首个完整、合法且下游可见的编码事件就绪后，才按现行合同 commit 200/SSE；对零可见事件时的非法首 frame、clean EOF 与 transport failure，保留各自错误与可重放分类。

下游业务输出提交后，不得 retry/fallback、拼接另一候选结果、回改已发送内容或补造终态掩盖失败。错误发现于提交前后时使用原有 HTTP/body 处置，不因统一 IR 而承诺提交后仍可修改 HTTP status。

### 5. 增量、有界与显式 buffering

正常 SSE 边 decode 边 encode，不默认聚合完整响应。只为 UTF-8、SSE field、参数增量及确定性规范化保留必要有界状态；达到合法可见事件的发送条件即发送，不增加固定 sleep。每事件、part/资源、累计 turn/body 和来源记录分别受限。

维持按下游消费拉取的背压、取消释放与既定 deadline。转换后不可见的事件仍推进同一个状态并释放 raw bytes，不累计无界 prefix，不重新渲染已消费事件。

已有显式 Responses SSE→JSON buffering 是受信 delivery 策略，不是通用聚合许可。它必须完成预算与终态验证后一次返回 JSON，并服从相同任务的静态语义；不自动开放 Chat SSE 聚合、Realtime 或新的流式媒体能力。

### 6. 媒体与来源限定事件

已支持任务中的媒体增量需有对应 typed event、格式、顺序和 item/part 资源关联；通用 opaque 不替代当前受支持的媒体语义。未知但合法的 Native 非终态事件，只按已批准的协议命名空间与预算规则保留为来源限定扩展，不能冒充终态，也不能获得跨 Provider 重放权限。

任务是否支持该媒体、事件或目标表示由任务合同决定；本页不把存在 Event 类型解释成公开能力，也不要求为未开放任务提前搭建框架。

## 替代方案与理由

| 方案 | 结论 |
|---|---|
| 校验 IR 后长期输出原始事件正文 | 不采用；语义变换不能约束实际输出 |
| 全部流先聚合再走静态 encoder | 不作为默认；改变延迟、资源与取消生命周期 |
| 出错后补 terminal 或重试拼流 | 不采用；破坏 commit 与客户端语义 |
| 有界增量 Event IR + 独立交付生命周期 | 采用；需维护状态一致性，但语义和执行权限明确 |

## 影响与落实

当前 Native 来源事件编码是迁移状态，不因文档调整立即移除。按[下一步目标](../implementation-plans/next-goal.md)逐语义切片对齐 codec、reducer、materializer、ingress 和测试；不能以静态内容已经迁移代替 Event 验收。

[离线验收方法](../../testdata/semantic-testing.md#9-provider-无关的任务-ircodec-验收)需要独立增量 encode 预期、分片等价正控制、EOF/终态负例、适用 materialize 一致性，以及提交前后错误、取消与预算见证。纯 codec 测试不证明真实网络背压、负载或 Provider 长流兼容。
