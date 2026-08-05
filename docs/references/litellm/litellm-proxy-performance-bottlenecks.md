# LiteLLM Proxy 性能瓶颈分析：Chat Completions 与 Responses

## 范围与证据

本文基于本地 `F:/codespace/litellm` 源码快照：`litellm_internal_staging` 分支，提交
`b3d05bd10b9a044ea08a1f1ce0e165ee5ba1ef35`。调用链细节见 [LiteLLM Proxy 调用链](litellm-proxy-call-chain-analysis.md)
。本文只分析性能，分三类标注：

**2026-08-01 当前模块级复核**：本地 `litellm_internal_staging` 已 fast-forward 至
`23de7a15d9d40006ee596e617475ba101d60c5e9`；共享请求处理、Responses route types、Prometheus/TTFT 与 failure-handler
模块仍可定位。下文的性能判断和行号仍是固定快照证据；用于其他版本或系统前必须重新测量并固定证据。

- **🔴 已确认瓶颈**：源码直接可证的阻塞/串行化/冗余访问。
- **🟡 条件性瓶颈**：取决于配置/部署，默认配置下可能命中。
- **🟢 设计良好**：已验证为非瓶颈，简述理由。

性能判断以「单请求关键路径（critical path）上的额外延迟」为度量：任何在 provider HTTP 往返之前/之中/之后发生的、非必要的同步等待、序列化或外部
I/O。

## 0. 两模式共享同一处理器，因此共享同一组瓶颈

`base_process_llm_request()`（`common_request_processing.py:1461`）是 Chat 与 Responses
的共同处理器。在认证、pre-call、route、cache、logging、spend 等共享阶段，两模式具有相同的性能特征；差异从 router 之后的 SDK 入口（
`acompletion` vs `aresponses`）开始，并包括 Responses 特有的 background/polling、container
ownership、Chat-bridge。因此下文先按共享阶段标注瓶颈，再在第 8 节给出模式差异。

## 1. 🔴 已确认瓶颈

### 1.1 `loop.run_in_executor(None, ...)` — async proxy 调 sync SDK

**证据**：

- `litellm/main.py:651`：`init_response = await loop.run_in_executor(None, func_with_context)`，
  `func = partial(completion, ...)`。
- `litellm/responses/main.py:537`：同构 `run_in_executor` 调同步 `responses()`。

**问题**：默认线程池 `concurrent.futures.ThreadPoolExecutor`（Python 默认 `min(32, os.cpu_count()+4)`）。在 Chat/Responses
流量混合下：

1. 线程池大小是进程级共享上限。一旦饱和，新请求排队等空闲线程，表现为 P99 延迟尖刺，且与 QPS 无关——只与「同时在 executor
   中的任务数」有关。
2. sync `completion()`/`responses()` 内部有同步 Redis 访问（见 1.2）、同步 cache key 构造（见 1.3）、provider 同步 HTTP 客户端（部分
   provider）。这些同步 I/O 占用线程，进一步压缩池子有效容量。
3. `contextvars.copy_context()` 每请求复制上下文（`:648`），在executor线程中 `ctx.run` 恢复——本身开销小，但意味着每请求一次跨线程切换。

**为何是瓶颈而非可接受**：proxy 是 async FastAPI，设计预期是单进程高并发。把核心 LLM 调用塞回同步线程池，等于把 async
优势（事件循环多路复用 I/O）让渡给线程池串行化。当 provider 调用本身是慢 I/O（LLM 推理 1-30s）时，线程池会被长任务占满，阻塞后续请求的
dispatch。

**影响面**：高并发（>线程池大小）时 P99 退化；流式响应尤其严重（一个流占线程数十秒）。

**缓解**：

- 使用 `concurrent.futures.ThreadPoolExecutor` 配置 `max_workers`，再通过事件循环的 `set_default_executor()` 设置默认
  executor。
- 更好的做法：让 `litellm.acompletion`/`aresponses` 在 async provider 路径上直接 `await` async provider handler，跳过
  executor。当前代码因为 `completion()` 是同步函数且 provider handler 混用 sync/async 才走 executor。

### 1.2 `RedisCache.set_cache` / `get_cache` 同步阻塞 event loop（在 sync 路径中）

**证据**：

- `redis_cache.py:400`：`self.redis_client.set(name=key, value=str(value), ex=ttl)`，同步 `redis-py` 客户端。
- `redis_cache.py:979`：`self.redis_client.get(key)`，同步。
- `DualCache.set_cache`（`dual_cache.py:118-128`）：先 in-memory `set_cache`，再 `self.redis_cache.set_cache`——若
  `DualCache` 被用于 async 路径但调用的是同步 `set_cache`，则 Redis 写同步阻塞。
- `DualCache.get_cache`（`dual_cache.py:153-180`）：in-memory miss 后 `self.redis_cache.get_cache`（同步）阻塞。

**调用路径**：`completion()`（sync，运行在 executor 线程）→ `LLMCachingHandler._sync_get_cache`（`caching_handler.py:289`）→
`litellm.cache.get_cache`（`caching.py:571`）→ `DualCache.get_cache` → `RedisCache.get_cache`。因为是 sync `completion()`
内调用，阻塞的是 executor 线程而非 event loop——但仍消耗 1.1 的线程池容量。更危险的是 `UserApiKeyCache`（`DualCache`
子类）在认证阶段被 async 代码直接调用：`IdentityStore.resolve` 在 async 路径若误用同步 `get_cache` 会阻塞 event loop。

**实际 async 路径**：`DualCache.async_get_cache`（`dual_cache.py:217`）正确使用 `await self.redis_cache.async_get_cache`（
`redis_cache.py:1066`，`await _redis_client.get`）。`LLMCachingHandler._async_get_cache`（`:150`）也用 async。 **问题在于 sync
`completion()` 不会走 async 路径**——它在 executor 线程里只能调同步 `RedisCache.get_cache`。所以 1.1 的 executor 是根因，1.2
是放大器。

**影响面**：每次 cache miss 命中一次同步 Redis 往返（通常 0.2-2ms，但 Redis 抖动时可达 50ms+），乘以请求数。

### 1.3 cache key 构造遍历全部 kwargs + hash（已验证为轻微开销，详见 🟡 2.6）

经源码验证，cache key 构造的实际开销远低于初判——messages/prompt/input/metadata 均被排除在 key 材料外，且参数白名单集合有
`@lru_cache`。完整分析移至 🟡 2.6。此处保留条目以说明曾作为疑似瓶颈被核查并降级。

### 1.4 认证阶段的多轮 cache→DB 串行/并行 fetch

**证据**：

- `_user_api_key_auth_builder`（`user_api_key_auth.py:1035`）顺序执行：end_user resolve → key cache lookup → key DB
  lookup → team refresh check → master key check。
- `_run_centralized_common_checks`（`:2089`）用 `asyncio.gather` 并行 fetch team/user/project/end_user/global_spend（
  `:2177-2254`）—— **这是好的设计**。
- 但每个 fetch 内部（如 `get_team_object`、`get_user_object`、`get_end_user_object`）各自是 cache→DB 两级，且各自的 cache
  miss 会串行执行 in-memory→Redis→DB。

**问题**：

1. 认证阶段在最坏情况下对同一 token 产生：1 次 in-memory key lookup（miss）+ 1 次 Redis key lookup（miss）+ 1 次 DB key
   query + 1 次 in-memory team lookup + 1 次 Redis team lookup + 1 次 DB team query + user/end_user/project/global_spend
   各自的 cache→DB。 **单请求认证可达 8-12 次外部访问**（cache 命中时大幅减少，但冷启动或 cache 过期后真实）。
2. `_get_hierarchical_router_settings`（`common_request_processing.py:1261`）在 pre-call 阶段再查一次 key/team 级
   router_settings，可能重复认证阶段已查的 team 对象。
3. `UserApiKeyCache` 的 TTL（`user_api_key_cache_ttl`，默认见 `get_management_object_ttl` `user_api_key_cache.py:153`）决定
   cache 命中率。TTL 过短 → 频繁 DB 回查。

**影响面**：认证阶段延迟在 cache 全命中时 <1ms，全 miss 时可达 10-50ms（多次 Redis + DB 往返）。这是请求 dispatch 前的纯开销。

### 1.5 pre_call_checks 遍历所有 callbacks，每个做 Redis RPM/TPM 检查

**证据**：

- `Router.async_routing_strategy_pre_call_checks`（`router.py:7266`）：
  `for _callback in litellm.callbacks: if isinstance(_callback, CustomLogger): await _callback.async_pre_call_check(deployment, ...)`。
- `_pre_call_checks`（`router.py:10005`）：读 `model_group_cache`（in-memory，local_only，`:10043`）做 RPM 检查。
- `router_utils/pre_call_checks/model_rate_limit_check.py`、`io_token_rate_limit_check.py`：基于 Redis incr 的
  per-minute/per-deployment 限流。

**问题**：

1. 每个 `CustomLogger` 的 `async_pre_call_check` 通常做 Redis `incr` + `ttl` 检查（RPM/TPM）。N 个 callback = N 次 Redis
   往返（串行，因为在 `for` 循环里 `await`）。
2. 这些检查在 `rpm_semaphore` 内执行（`:2827`），意味着持有 semaphore 期间做 Redis I/O，进一步限制并发。

**影响面**：每请求 N× (Redis 往返)。N=1-3 时可接受（0.5-3ms），N>5 且 Redis 延迟高时显著。

## 2. 🟡 条件性瓶颈

### 2.1 Responses Chat-bridge 的双重转换开销

**证据**：`responses/litellm_completion_transformation/handler.py:23`、`transformation.py`。

当 provider 无 native Responses config 时，Responses 请求被转为 Chat completion 请求，调 `litellm.completion`，再把 Chat
响应转回 Responses。这意味着：

- 请求侧：Responses input → Chat messages（`:379`）。
- 响应侧：Chat ModelResponse → ResponsesAPIResponse（`:1588`）。
- 流式侧：Chat SSE → Responses SSE 状态机（`streaming_iterator.py:51`）。

**条件**：仅当 provider 不支持原生 Responses（如 Anthropic、Bedrock 等通过 Chat 桥接）时命中。OpenAI 原生 Responses 走
`responses/main.py` 的 native 路径，无此开销。

**影响**：非流式额外 1-3ms CPU（转换 + schema 校验）；流式额外每个 chunk 的状态机推进开销，但通常 <provider 网络延迟。

### 2.2 Responses background + polling 的 Redis 状态写入

**证据**：`endpoints.py:110-191`、`response_polling/background_streaming.py`、`polling_handler.py`。

background 模式下，`background_streaming_task` 在后台流式完成并持续 `polling_handler.update_state` 写 Redis。每 chunk 或每
N chunk 一次 Redis SET。若 chunk 粒度细 + 流长，Redis 写次数可观，但这是 background 模式的固有成本且不阻塞客户端（客户端立即拿
polling_id 返回）。

### 2.3 `_wrap_responses_stream_for_container_ownership` 的 stream 结束同步写

**证据**：`common_request_processing.py:2038-2086`。流式 Responses 在 stream 结束时同步
`await record_container_owners_from_responses_response`（DB 写 `LiteLLM_ManagedObjectTable`）。这是 stream
末尾的阻塞点，延迟响应关闭。仅当流中产生了 code-interpreter container 时触发。

### 2.4 guardrail 的同步 `threading.Thread` 调用

**证据**：`router.py:7298-7301`、`:7321-7324`：`threading.Thread(target=logging_obj.failure_handler, args=...).start()`。

在 pre_call_check 失败时，用 `threading.Thread` 调同步 `failure_handler`。这是 fire-and-forget，不阻塞响应，但：

1. 每失败请求创建一个线程，高频失败时线程创建开销累积。
2. `asyncio.create_task(logging_obj.async_failure_handler(...))` 与 `threading.Thread` 并存（`:7290` + `:7298`
   ），双路径日志可能重复或乱序。

### 2.5 OTel span 创建开销

**证据**：`user_api_key_auth.py:2513` `with phase_span(f"auth {route}")`、`:1074` `with tracer.trace(...)`、多处
`tracer.trace`。每请求多个
span（auth、pre_db_read、get_key_object、jwt_auth_builder、get_end_user_object、get_key_object_from_db）。OTel export 若为同步
batch，span 创建本身 cheap 但 export 可能在后台阻塞；若每请求 5-10 span 且 exporter 慢，有累积。

### 2.6 cache key 构造遍历全部 kwargs + hash（由 1.3 降级）

**证据**：`caching.py:329-380` `get_cache_key()`；`model_param_helper.py:42-43`、`:58`、`:171`。

**已验证事实**：

1. `_get_all_llm_api_params()`（`model_param_helper.py:59`，`@lru_cache(maxsize=1)`）返回的参数白名单 **已排除**
   `messages`、`prompt`、`input`（`get_exclude_params_for_model_parameters` `:42-43`）和 `metadata`（`_get_exclude_kwargs`
   `:171`）。因此 cache key 材料不含大消息体，原担心的「messages 序列化进 key」不成立。
2. `_SEMANTIC_CACHE_SCOPE_EXCLUDED_PARAMS`（`caching.py:297`）在 semantic cache 场景对 messages/prompt/input 再排除一次。
3. 每次 cache 查找都遍历 `kwargs` + 对白名单参数 `f"{param}: {value}"` 拼接 + 最终 hash。对 20-30 个白名单参数，CPU 约
   10-50µs/请求。

**为何 🟡 而非 🔴**：开销真实存在但量级小（10-50µs），相对 Redis 往返（0.2-50ms）可忽略。仅在追求亚毫秒级 cache hit 延迟时占比可见。
`lru_cache` 已优化集合构造。

### 2.7 `add_litellm_data_to_request` 的 metadata 深拷贝

**证据**：`litellm_pre_call_utils.py:1547` 执行 `copy.deepcopy(data["metadata"])`，将清洗后的请求 metadata 保存为
`requester_metadata`。同一函数在 `:1537-1539` 对请求 body 建立的是顶层 dict snapshot，不是深拷贝整个 `data`；因此
`messages` 不会因这里的 `deepcopy` 被递归复制。

**条件与影响**：常规小型 metadata 的开销通常可忽略；仅当 client 传入异常大或深层嵌套的 metadata 时，深拷贝才可能成为热路径
CPU/内存压力。应先按 metadata 大小和该段耗时测量，再决定是否引入大小上限、允许字段白名单或浅拷贝策略；不能把它列为与线程池饱和同级的已确认瓶颈。

## 3. 🟢 设计良好（已验证非瓶颈）

### 3.1 spend log 批量队列

**证据**：`utils.py:2777-2778` `spend_log_transactions: List` + `_spend_log_transactions_lock`；`:5171`
`update_spend_logs` 定时 flush；`:5130` `ProxyUpdateSpend` 用 `db.tx(timeout=60s)` + `batch_()`。

per-request 只 append 内存 list（O (1)），不触 DB。flush 每分钟 + 队列监控任务，batch 1000 行/批。这是正确的异步批处理设计。

### 3.2 deployment client 复用

**证据**：`router.py:9965` `_get_client` 按 `model_id`+`client_type` 从 `self.cache`（`local_only=True`）取缓存的
`AsyncOpenAI`。httpx 连接池跨请求复用，避免每请求 TLS 握手。仅在 `api_key` 动态变化时放弃复用（`:3128`）。

### 3.3 `_run_centralized_common_checks` 的并行 gather

**证据**：`user_api_key_auth.py:2177-2254` 用 `asyncio.gather` 并行 fetch team/user/project/end_user/global_spend，
`_safe_fetch` 隔离每 fetch 错误。把 5 个串行 DB 访问并行化，是最优结构。

### 3.4 master key 缓存异步写入

**证据**：`user_api_key_auth.py:1600` `asyncio.create_task(_cache_key_object(...))`。master key 命中后异步写缓存，不阻塞响应返回。

### 3.5 `_pre_call_checks` 的 shallow copy 优化

**证据**：`router.py:10023-10025` 注释明确：`list(healthy_deployments)` 替代 `deepcopy`，100x+ faster。表明已识别并修复
deepcopy 瓶颈。

## 4. 瓶颈优先级排序

按「默认配置下的命中概率 × 影响程度」排序：

| 排名 | 瓶颈                                | 命中概率                     | 影响程度          | 根因                               |
|------|-------------------------------------|------------------------------|-------------------|------------------------------------|
| 1    | 🔴 1.1 run_in_executor 线程池       | 高（所有请求）               | 高（高并发退化）  | sync SDK + executor 架构           |
| 2    | 🔴 1.4 认证阶段多轮 cache→DB        | 高（cache miss 时）          | 中（10-50ms）     | 多对象串行 fetch + TTL             |
| 3    | 🔴 1.2 sync RedisCache 在 sync 路径 | 高（cache miss 时）          | 中（0.2-50ms/次） | sync completion 调 sync cache      |
| 4    | 🔴 1.5 pre_call_checks N×Redis      | 中（取决于 callback 数）     | 中（N×Redis）     | for 循环串行 await                 |
| 5    | 🟡 2.1 Responses Chat-bridge        | 中（非 OpenAI provider）     | 低（CPU 1-3ms）   | 双重转换                           |
| 6    | 🟡 2.6 cache key 构造               | 高（每请求）                 | 低（10-50µs CPU） | 全量遍历 + hash（messages 已排除） |
| 7    | 🟡 2.3 container ownership 同步写   | 低（仅 code-interpreter 流） | 低（stream 末尾） | DB 写在 stream close               |

## 5. 测量建议

在动手优化前，先量化每个瓶颈的真实贡献：

1. **executor 饱和度**：`asyncio.get_event_loop()._default_executor._max_workers` 与运行中任务数。压测时监控线程池队列长度。
2. **认证延迟**：在 `user_api_key_auth` 入口与出口打点（`start_time`/`end_time`），按 cache hit/miss 分桶。
3. **cache key 构造耗时**：在 `caching.py:329` 入口与 `_get_hashed_cache_key` 出口打点。
4. **Redis 往返次数**：在 `redis_cache.py` 的 `get_cache`/`set_cache`/`async_get_cache`/`async_set_cache` 加
   counter，每请求统计。
5. **pre_call_checks 耗时**：在 `router.py:7266` 入口与循环后打点，按 callback 数分桶。
6. **spend log 队列深度**：监控 `len(prisma_client.spend_log_transactions)` 与 flush 频率。

## 6. 优化方向（按 ROI 排序）

1. **让 `litellm.acompletion`/`aresponses` 在 async provider 路径上直接 `await`**，跳过 `run_in_executor`。这是最大
   ROI，但需重构 sync `completion()` 与 async provider handler 的边界。
2. **认证阶段预热/合并 fetch**：在 `_user_api_key_auth_builder` 已查 team 后，`_run_centralized_common_checks` 复用该 team
   对象而非重新 fetch；`_get_hierarchical_router_settings` 同样复用。
3. **cache key 预计算或缓存**：若 `preset_cache_key` 已存在则跳过全量构造（`caching.py:336` 已支持）；在 router 层为常见
   model+messages 组合预计算 key。
4. **pre_call_checks 批量化**：将多个 callback 的 Redis 计数/TTL 操作合并为保留原子语义的 Redis pipeline 或 Lua 脚本；
   `MGET` 不能替代 `INCR` 类写入。
5. **`UserApiKeyCache` TTL 调优**：默认 TTL 偏短则频繁 DB 回查，按部署实际调整。

## 7. 两模式性能差异小结

| 维度                          | Chat Completions                                                     | Responses                                           |
|-------------------------------|----------------------------------------------------------------------|-----------------------------------------------------|
| 认证/pre-call/cache/post-call | 非 background 请求走相同的共享处理路径（`base_process_llm_request`） | 同左                                                |
| SDK 入口                      | `run_in_executor(sync completion)` 🔴1.1                             | `run_in_executor(sync responses)` 🔴1.1（对称瓶颈） |
| 协议转换                      | 无                                                                   | 🟡 Chat-bridge 双重转换（条件性）                   |
| background/polling            | 无                                                                   | 🟡 Redis 状态写入（background 模式固有）            |
| container ownership           | 无                                                                   | 🟡 流末 DB 写（仅 code-interpreter）                |
| 流式 cache                    | 逐 chunk 累积写                                                      | 同左（经 Chat-bridge 时多一层 SSE 状态机）          |

**结论**：两模式性能瓶颈几乎完全重叠，因为共享处理器与 SDK 架构。Responses
的额外开销（Chat-bridge、background、container）都是条件性且非主路径，不构成模式间显著差异。优化 1.1-1.5 对两模式同等受益。

## 8. 附：关键证据行号速查

| 瓶颈                           | 文件:行                                                                    |
|--------------------------------|----------------------------------------------------------------------------|
| run_in_executor                | `litellm/main.py:651`、`litellm/responses/main.py:537`                     |
| sync Redis set                 | `litellm/caching/redis_cache.py:400`                                       |
| sync Redis get                 | `litellm/caching/redis_cache.py:979`                                       |
| DualCache sync get             | `litellm/caching/dual_cache.py:153-180`                                    |
| cache key 构造                 | `litellm/caching/caching.py:329-380`                                       |
| 认证 key cache→DB              | `litellm/proxy/auth/user_api_key_auth.py:1486`、`:1656`                    |
| 集中检查并行 fetch             | `litellm/proxy/auth/user_api_key_auth.py:2177-2254`                        |
| router_settings 再查           | `litellm/proxy/common_request_processing.py:1261`                          |
| pre_call_checks 遍历 callbacks | `litellm/router.py:7266-7325`                                              |
| spend log 批量队列             | `litellm/proxy/utils.py:2777`、`:5171`、`:5130`                            |
| client 复用                    | `litellm/router.py:9965-10003`                                             |
| Responses Chat-bridge          | `litellm/responses/litellm_completion_transformation/handler.py:23`、`:62` |
| container ownership wrap       | `litellm/proxy/common_request_processing.py:2038-2086`                     |
