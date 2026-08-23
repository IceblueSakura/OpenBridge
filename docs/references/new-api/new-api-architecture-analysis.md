# new-api 架构与产品形状

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | `QuantumNous/new-api` @ `2d8e50bf36e94200b809dfb39e73624ec48b1e23` |
| Last reverified | 2026-08-24，本地只读源码复核 |
| Scope | 产品定位、模块边界、HTTP 请求主链、控制面与数据面关系 |
| Evidence boundary | 静态源码；未启动服务、数据库、Redis、前端或真实 Provider |
| Recheck trigger | 路由、`controller.Relay`、`RelayInfo`、Adaptor、模块拆分或许可证变化时 |

## 1. 产品定位

README 将项目称为“Next-Generation LLM Gateway and AI Asset Management System”，并明确覆盖组织级认证、多模型管理、
用量分析、成本核算和私有部署：`README.md:5-7`、`README.md:56-62`。

它不是单一反向代理，而是一个 Go 单体中的控制面与数据面：

- 数据面：鉴权、限流、模型/渠道选择、协议转换、上游调用、stream 转发、usage 提取和结算；
- 控制面：用户、token、权限、渠道、模型、价格、订阅、支付、日志、后台任务和 React 管理端；
- 状态层：GORM 数据库、Redis 和进程内缓存；
- Provider 层：`relay/channel/*` 下的原生或 OpenAI-compatible adaptor。

初始化和后台组件集中在 `main.go:284-367`，HTTP server 创建位于 `main.go:173-216`。这是一种面向运营平台的集成式
部署形状，不是独立、不可变的数据平面。

## 2. 公共协议面

`router/relay-router.go:69-227` 暴露或代理：

- OpenAI Chat Completions、Completions、Responses、Embeddings、Images、Audio；
- Anthropic Messages；
- Gemini generateContent/streamGenerateContent；
- Rerank、Realtime WebSocket；
- Midjourney、Suno 和视频等任务型接口。

`/v1` 路由组统一叠加 route tag、系统性能检查、token auth、模型限流和渠道分发：
`router/relay-router.go:69-85`。多种文本协议入口最终复用 `controller.Relay`：
`router/relay-router.go:87-151`。

## 3. 请求主链

典型文本请求的数据流为：

```text
HTTP request
  → route middleware / TokenAuth / rate limit / Distribute
  → 按入口协议解析 DTO 并校验
  → 创建 RelayInfo
  → token 估算、价格计算和预扣
  → group + model 选择渠道
  → 模型映射、API type、base URL、credential
  → retry loop
  → Provider Adaptor 请求转换或 passthrough
  → 上游 HTTP/SSE/WebSocket
  → 响应转换和 usage 提取
  → 实际结算、退款或补扣
  → consume log、metrics、渠道错误处理
```

主要证据：

1. 请求解析按 relay format 分发：`relay/helper/valid_request.go:21-57`；
2. `RelayInfo` 汇总请求级状态并初始化转换链：`relay/common/relay_info.go:83-172`、`relay/common/relay_info.go:579-668`；
3. token 估算、价格和预扣位于 `controller/relay.go:112-171`；
4. 渠道选择及 retry loop 位于 `controller/relay.go:184-244`；
5. 普通文本转换、字段调整和发送位于 `relay/compatible_handler.go:42-220`；
6. 成功结算、失败退款及渠道错误处理位于 `controller/relay.go:173-182`、`controller/relay.go:363-405`。

## 4. `RelayInfo` 的角色

`RelayInfo` 是一次请求跨阶段共享的 mutable session state，包含：

- 原始模型、映射后的模型、入口 relay format 和 relay mode；
- 用户、token group、渠道、API key、base URL；
- stream 状态、估算 token、usage 和计费会话；
- retry index、last error、渠道 affinity；
- `RequestConversionChain` 和 `FinalRequestRelayFormat`。

转换链按格式记录，例如 `openai → openai_responses`，并避免相邻重复：
`relay/common/relay_info.go:640-680`。消费日志把它转成管理员可读名称：
`service/log_info_generate.go:209-245`。

这种集中上下文减少了函数参数数量，但也使协议、路由、计费和观测共享同一 mutable object；理解任一阶段时必须检查它此前可能被
哪些 handler/adaptor 修改。

## 5. Adaptor 边界

统一接口定义在 `relay/channel/adapter.go:16-33`。Adaptor 负责：

- 构造 URL 和 header；
- 将 OpenAI、Responses、Claude、Gemini、Embedding、Audio、Image 请求转成上游形状；
- 执行请求；
- 将 JSON 或 stream 响应转回客户端格式并提取 usage。

`relay/relay_adaptor.go:56-133` 按 `ApiType` 创建 adaptor。协议格式转换正在下沉到独立 `relaykit/relayconvert`，但
Provider endpoint、认证、方言修正和响应 handler 仍由 channel package 负责。

## 6. 控制面特征

渠道、价格、用户和权限来自数据库及动态配置。配置模块通过统一 manager 注册、读取和保存：
`setting/config/config.go:13-89`。进程还周期同步授权、渠道、凭据、订阅和实例状态：`main.go:108-152`。

这些机制适合运营型系统，但增加了：

- 数据库状态与进程内快照的同步窗口；
- 管理写入与请求读取的并发治理；
- 多实例调度、租约和缓存失效；
- 数据面行为受在线控制面变化影响。

## 7. 证据结论

new-api 的主要价值是宽协议面、Provider 适配经验、渠道运营和完整账务生命周期。其实现形状同时带有明显单体特征：主控制器承担
较多编排，普通 relay 和任务 relay 存在相似但独立的 retry/settlement 路径，全局动态缓存和配置增加一致性成本。

这些事实说明它适合作为 Provider 行为、转换 fixture、渠道巡检和运营边界的参考实现；不能仅凭功能覆盖推导其每一条转换都具有
无损语义，或其在线控制面适合所有 gateway 架构。
