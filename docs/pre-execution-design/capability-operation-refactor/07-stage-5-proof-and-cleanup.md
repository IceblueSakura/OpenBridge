# 07：阶段 5——纵向证明、清理与收口

## 目标

用一个真实需要的新增能力纵向证明目标结构，而不是继续做横向抽象；随后删除全部 legacy 和迁移残留，恢复单一事实 owner。

本阶段的具体功能必须在执行前重新选择。当前推荐默认是 typed file input 的有限 Native slice，因为现有 `file_input: bool` 是最明显结构缺口；若届时用户优先需要标准 Images 或 Audio endpoint，应重新评估并替换证明切片。

## 候选证明切片：Typed file input

首个切片建议只包括：

- Chat `file` inline data；
- Responses `input_file` inline data 与受限 remote HTTPS URL；
- encoding、media type、filename、part count、URL 和 inline byte limits；
- Native wire fidelity；
- Bridge、resource ID、下载、解析和转换全部 fail closed。

明确排除：

- Files/Uploads/Vector Stores/File Search 生命周期；
- `file_id` issuer/owner/affinity；
- PDF/text extraction、OCR、病毒扫描、下载代理或跨 Provider migration。

## 纵向证明必须经过

```text
OpenAPI/router/auth/body limit
→ operation analyzer
→ typed request facts
→ Public Model preflight
→ fixed RoutePlan
→ Provider operation adapter
→ loopback upstream
→ JSON/SSE response contract
→ attempt/commit/cancel/observability
```

不能只测试 profile constructor、adapter body 或 mock compiler。

## 先失败测试

- Models v2 公开 typed file profile；
- 合法 inline/remote request exact egress；
- source one-of、encoding、filename、MIME、detail、part、URL、encoded/decoded budget 逐项拒绝；
- `file_id` 无 affinity 时 zero egress；
- Bridge candidate 关闭 file capability；
- 超 replay budget 的合法请求只执行首个 attempt；
- URL query、filename、Base64 和错误上下文不进入普通 telemetry；
- cancel 和首输出 commit 边界正确。

## Legacy 清理

纵向切片通过后执行全仓库删除审查：

- old capability/module/type names；
- compatibility conversion 和 unused alias；
- old Models v1 fixture/schema；
- registration 手工 media mutation；
- fixed operation fields；
- duplicate test builders 和 orphan fixtures；
- stale architecture/status/requirement links。

禁止以 TODO、feature flag 或 dead branch 保留旧路径。

## 退出门

- 当前所有 operation 与新切片的 focused tests 全绿；
- full Rust、corpus（若改动）、OpenAPI、link 和 `git diff --check` 全绿；
- implementation status 记录实际完成事实与命令；
- requirements 只保留真实产品合同；
- 本设计包仍标记为设计，不改写成实施历史；
- `current-focus.md` 恢复为空。

## 后续扩展规则

完成证明后，每个新 operation 仍按单独 current focus 纵向实施：定义 wire/limits/errors → RED fixture → operation profile → registry interface → pipeline/adapter → provider evidence。不得因为框架存在就批量打开未验证 capability。
