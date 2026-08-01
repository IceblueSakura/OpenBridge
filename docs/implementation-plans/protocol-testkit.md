# Mock Server/Client 测试工具设计

## 状态

**独立测试工具。** 当前保持独立于 OpenBridge runtime；只有具体行为进入当前开发焦点时，才由 runner 将稳定 scenario 接入被测进程。工具存在不表示运行时已经实现 Bridge。

## 1. 定位

该工具链主要服务于 OpenBridge 后续黑盒开发测试，但当前保持独立于 OpenBridge runtime：

```text
Mock Client -> future SUT -> Mock Server
```

CLI、scenario/plan、Server/Client 行为、observation 字段和 loopback 示例以 [Testkit 指南](../../tools/corpus/README.md) 为准；本文件只保留设计定位和后续集成边界。

当前工具只实现两端协议行为、运行计划编译与 observation，不加载 OpenBridge 配置、不启动 OpenBridge、不引用 Rust crate，也不判断 routing、fallback 或转换是否正确。后续 runner 才负责把两端连接到被测进程并比较 canonical oracle。

## 2. 组件边界

### 2.1 Incremental SSE parser

- 直接消费任意 bytes chunk，不假设 socket read 与生成 variant 边界一致；
- 支持 CRLF、LF、CR、comment、多个 `data:`、UTF-8 跨 chunk 和一个 chunk 多 event；
- EOF 不派发缺少空行的最后 event；
- 同时保留 SSE `event:` 与 JSON `type`，不以其中一个覆盖另一个；
- 单独标记 terminal 和 event/type conflict。

### 2.2 Server scenario

`build-server-scenario` 从一个有上游 attempt 的 canonical case 编译自包含 scenario：

- 上游 method/path；
- expected request JSON 和 canonical body hash；
- HTTP status 与 headers；
- canonical 或 generated wire chunks；
- chunk delay、abort delay 和 `complete`/`abort` termination。

scenario 是 `testdata/runtime/` 派生产物，不是新的 oracle。

多个 scenario 可按顺序编译为 server suite。suite 中每个普通请求原子认领一个 exchange；健康检查、非法 JSON 和未知 endpoint 不消耗 exchange。

### 2.3 Mock Server

- 使用 `asyncio + h11` 提供 HTTP/1.1 数据面；
- 默认仅绑定 `127.0.0.1`；
- 一个进程可执行单 scenario 或有序多-exchange suite；
- `/health` 与 `/healthz` 返回状态和剩余 exchange 数；
- 非法 JSON 返回 OpenAI 风格 400，未知 endpoint 返回 404；
- 按 scenario 顺序写入 body chunks；
- 支持正常 HTTP message 完成和输出后的异常连接终止；
- 记录 method、target、headers、raw body、解析后的 JSON、hash、结束方式和 timing；
- 写盘前脱敏 Authorization、Cookie 和 API-key 类 headers。

### 2.4 Client plan 与 Mock Client

`build-client-plan` 从 canonical `client_request` 编译：

- client protocol URL；
- 请求 headers/body/hash；
- stream、timeout 和 cancellation point。

Mock Client：

- 使用 `asyncio + h11`，不依赖 OpenAI SDK；
- 默认不重试；
- 分别记录 HTTP envelope、raw body chunks、逻辑 SSE events 与 terminal；
- 区分普通 response、HTTP error response、logical EOF、transport error 和 cancellation；
- HTTP status 优先于 Content-Type 分类，`4xx/5xx + text/event-stream` 仍是 HTTP error response；
- 可在第 N 个逻辑 event 后终止连接。

### 2.5 Observation

Server 与 Client 分别输出 observation。工具不在两端内部做 OpenBridge 产品断言；后续 runner 应比较：

1. Server 收到的上游请求与 `expected_upstream_request`；
2. Client 收到的 envelope/events 与 expected client artifact；
3. identity、ordering、terminal、attempt 与 fallback 不变量；
4. transport failure phase 和 downstream output commit point。

## 3. 分片测试边界

generated chunk 是确定性的 parser/write 输入，不是 TCP packet 承诺。操作系统仍可能合并或重新拆分 socket 数据：

- parser 测试逐个喂入全部 306 个生成 variant；
- black-box 测试只断言逻辑 event、顺序、bytes hash 和结束方式；
- 不断言 OpenBridge 每次 socket read 的边界。

## 4. 当前明确不做

- OpenBridge 配置生成、进程启动、健康检查和日志采集；
- OpenBridge adapter 或 conversion oracle runner；
- HTTPS、HTTP/2、proxy、WebSocket；
- 运行时动态增删 scenario 的控制面和并发 worker；
- stall、带宽限制、背压和长时间负载；
- OpenAI SDK 兼容测试；
- 真实 Provider 或客户端验收。

## 5. 后续进入 runner 前的条件

- scenario、client plan 和 observation schema 经过至少一次兼容演进；
- Mock Server/Client CLI loopback 可独立运行；
- 正常 terminal、HTTP error、输出后 abort 和 event cancellation 均有测试；
- 敏感 header 不进入 observation；
- parser 对全部生成 variant 通过；
- 工具可以在没有 OpenBridge binary 的环境安装和测试。
