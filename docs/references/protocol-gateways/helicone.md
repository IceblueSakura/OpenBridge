# Helicone AI Gateway Rust runtime、routing 与 observability 调研

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | [`Helicone/ai-gateway` `main` @ `9649b27bdc9fb0907d359e899894102a15f3a085`](https://github.com/Helicone/ai-gateway/tree/9649b27bdc9fb0907d359e899894102a15f3a085) |
| Last reverified | 2026-08-30，本地只读 Rust 源码与测试源码复核 |
| Scope | Router config、weighted/latency balancing、retry、health、cache、metrics、Provider endpoint mapping 与 mock tests |
| Evidence boundary | 未构建或启动 gateway、Redis/Postgres/Provider mock；静态源码不证明生产性能、Provider capability等价或故障恢复质量 |
| Recheck trigger | router/balance/retry、Provider endpoint、body mapping、health monitor、cache/metrics 或 GPL license 变化时 |

## 1. Architecture

Helicone 是 Rust/Tower/Axum runtime gateway。Router config将load balance、model mapping、cache、retry、rate limit和per-Provider base URL集中到一个配置对象：`ai-gateway/src/config/router.rs:38-52`。Provider endpoint枚举与request/response mapper分开，metrics和error handling以Tower layer/service组合。

它的核心参考价值是runtime request lifecycle，而不是富语义protocol IR。Provider集合按 `EndpointType` 建立balancer；默认Chat balancer直接包含OpenAI、Anthropic和Gemini：`ai-gateway/src/config/balance.rs:14-30`。没有看到在balancer前对tools、reasoning、structured output、state等value-sensitive capability做typed交集验证。

因此“同属Chat endpoint”在该架构中足以进入同一候选集合，但不能由源码推导候选semantic等价。面向固定公共合同的gateway，应先编译capability-safe candidate set，再在集合内部借鉴其runtime policy。

## 2. Routing 与 health

`BalanceConfigInner`支持Provider weighted、balanced latency、model weighted和model latency：`ai-gateway/src/config/balance.rs:128-182`。Router validation验证权重和为1：`ai-gateway/src/config/router.rs:54-81`。这适合稳定候选集合内的policy选择；不负责把不兼容Provider动态筛出。

源码和integration tests覆盖load balance、weighted balance、health monitor、single-provider和direct proxy。测试mock可为OpenAI、Anthropic、Gemini、Ollama、Bedrock和Mistral注入独立latency与stub：`ai-gateway/src/tests/mock.rs:19-151`。这种隔离loopback测试形状可用于验证attempt order、health state和fallback，不应混入protocol semantic fixture。

## 3. Retry 与 fallback

Router可配置constant或exponential+jitter backoff、delay和max retries：`ai-gateway/src/config/retry.rs:9-68`。`RetryWithResult`允许基于完整 `Result` 判断retry、调整delay并通知observer：`ai-gateway/src/utils/retry.rs:15-113`。

该generic retry utility本身不知道stream commit、continuation affinity、credential secrecy或semantic fidelity。采用时必须由上层提供：

- retryable error taxonomy；
- first-visible-event commit gate；
- candidate capability等价；
- stateful request禁用fallback；
- attempt级observation和资源cleanup。

## 4. Streaming 与 body lifecycle

body wrapper可分叉流供reader/observer使用，并在首个body bytes上发送TTFT信号：`ai-gateway/src/types/body.rs:55-85`。这是低侵入TTFT测量的参考，但不等于semantic first event；SSE comment、metadata、usage或error可能不是首个业务输出。

Helicone的重点是transparent gateway transport和runtime telemetry，没有独立canonical Event IR。对stream retry而言，仅以首个bytes作为commit boundary可能过早；严格protocol gateway需要先decode event并判断first-visible semantic output。

## 5. Cache 与 observability

Router配置可按router启用cache，tests分别覆盖memory/Redis cache和不同router组合。cache命中、router选择和seed/bucket属于runtime policy，不应进入generation semantic IR；cache replay仍需保持目标protocol terminal、usage和Provider-private state policy。

error layer将错误类型计入OpenTelemetry counter：`ai-gateway/src/utils/handle_error.rs:93-99`。Provider key、router、latency和TTFT也有metrics。可借鉴的是attempt/router/transport metrics owner分离；不能从metric存在推导semantic conversion正确。

## 6. 可吸收测试资产

建议自主重写：

1. capability-safe固定候选内weighted/latency routing；
2. unhealthy candidate跳过与恢复后重新加入；
3.retry只覆盖允许的错误和次数，并记录每次attempt；
4. pre-commit失败可fallback，post-commit失败不可拼接第二Provider；
5. cache命中不产生upstream attempt且保持terminal/usage policy；
6. TTFT区分first byte、first SSE event与first visible semantic event；
7. per-router rate/cache/retry配置不泄漏到其他router；
8. mock Provider使用隔离port、事件和bounded timeout。

Helicone使用GPL-3.0。默认只借鉴测试形状和独立场景；复制代码或fixture需要单独审查GPL边界。

## 7. Lessons

### Adopt

- Router policy、Provider endpoint、retry、health、cache和metrics模块化；
- deterministic multi-Provider mock与latency/failure injection；
- attempt结果驱动的retry predicate和observer。

### Adapt

- 在runtime balancer之前加入已验证capability-equivalent candidate set；
- 将first-byte TTFT与first-visible semantic commit分离；
- cache key/result加入protocol、semantic requirements、state affinity和fidelity policy。

### Avoid

- 以共同EndpointType替代capability equivalence；
- 让generic retry跨越stream commit或stateful continuation；
- 把runtime routing/health字段塞进canonical generation IR。

### Open Questions

- streaming response失败在哪个layer阻止后续retry/fallback；
- latency/health采样是否区分model、Provider、endpoint和request semantic；
- cache是否保存/重放streaming与Provider-private metadata；
-错误映射是否稳定区分Provider、transport、policy和client failures。
