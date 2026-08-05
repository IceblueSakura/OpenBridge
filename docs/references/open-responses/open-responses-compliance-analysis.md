# Open Responses Compliance 测试调研

## 状态与来源

- 在线复核日期：2026-07-26
- 本次在线检查未固定 commit。
- 来源：[Open Responses](https://www.openresponses.org/)、[Acceptance Tests](https://www.openresponses.org/compliance)、[repository](https://github.com/openresponses/openresponses)、[`compliance-tests.ts`](https://github.com/openresponses/openresponses/blob/main/src/lib/compliance-tests.ts)、[CLI](https://github.com/openresponses/openresponses/blob/main/bin/compliance-test.ts)

## 观察事实

- Open Responses 是以 OpenAI Responses API 为基础的独立开放规范与生态，不声明与官方 API 完全相同。
- 复核时 `testTemplates` 可见 17 个 HTTP/SSE 与 WebSocket 场景。
- HTTP 场景包括文本、assistant phase、schema、SSE、system prompt、单 function tool、image、multi-turn、compact 和缺失 model。
- WebSocket 场景包括同连接连续响应、`store:false` continuation、重连 recovery、缺失 previous response 与 compact 新链。
- stream validator 解析 event schema 并要求 terminal response；terminal 前关闭连接会记录错误。

## 覆盖与边界

它对 Responses schema、terminal 与 continuation 的黑盒 acceptance 较强；function tool 场景较浅，不覆盖复杂并行调用、arguments 任意分片、tool result 往返或 Chat 转换。其规范版本必须与 OpenAI 官方 Responses 分开记录。

