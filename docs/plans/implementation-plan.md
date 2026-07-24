# 当前阶段实施计划：C0 范围与客户端契约

## 状态

**Active。** 本计划只覆盖 C0，不包含 C1–C6 或增强阶段的工作包。

阶段目标、测试和退出条件以 [C0 阶段契约](../phases/00-scope-and-client-contracts.md)为准；产品与跨阶段约束以[核心需求](../requirements/proxy-requirements.md)和[阶段交付与研究需求](../requirements/delivery-requirements.md)为准。

## 1. 本阶段结果

完成 C0 时，应获得一组可复现、可供 C1 直接使用的输入：

- 冻结的首版产品范围和非目标；
- 一个固定 Codex 版本、平台和 custom Provider 配置；
- 一个固定 Hermes Agent 版本、平台和 Chat 配置；
- 两条 P0 Native Path 的成功、错误、EOF、partial stream、cancel 和 tool-loop corpus；
- fixture 脱敏规则、目录结构、重跑命令和证明边界；
- C0-01 至 C0-06 的逐项 gate review。

C0 的结果是“实现输入已固定”，不是“Native Path 已通过真实客户端验收”。后者属于 C1。

## 2. 固定范围

### 范围内

- 完善需求层级、阶段状态和单阶段计划规则；
- 固定目标客户端版本、安装来源、平台和最小配置；
- 明确 Codex Responses HTTP/SSE 与 Hermes Chat 的 wire contract；
- 定义并收集 C0 阶段所需的脱敏 corpus；
- 记录每个实验能够证明和不能证明的事项；
- 对照 C0 测试 ID 完成阶段评审。

### 范围外

- 修改 `src/` 中的运行时代码；
- 以发现的兼容性问题为由直接修复 Native Path；
- 第二 Provider Family、Protocol Bridge、continuation ledger；
- OAuth、Hosted Tool/MCP、usage、health、UI；
- 为 C1 或后续阶段编写实现切片；
- 把 SDK/mock fixture 结论提升为真实客户端或真实 Provider 验收。

若 corpus 暴露运行时缺陷，只记录复现、影响需求和 C1 输入；除非先修改 C0 阶段契约，否则不在本计划中实施代码修复。

## 3. 执行输入

| 输入 | 当前值 | 缺失时处理 |
|---|---|---|
| Codex 版本、来源和平台 | `【需根据实际情况完善】` | C0-01/C0-02 保持未完成，不使用“最新版”代替。 |
| Codex custom Provider 配置 | `wire_api = "responses"`、`supports_websockets = false`；其余 `【需根据实际情况完善】` | 保存无 secret 模板和实际 transport 诊断。 |
| Hermes Agent 版本、来源和平台 | `【需根据实际情况完善】` | C0-03 保持未完成。 |
| Hermes Chat Provider 配置 | `【需根据实际情况完善】` | 保存无 secret 模板。 |
| 授权 Responses/Chat Provider 与模型 | `【需根据实际情况完善】` | 不宣称真实 Provider corpus 已具备。 |
| 脱敏规则和 artifact 保存位置 | `【需根据实际情况完善】` | 含 secret、cookie 或私人 prompt 的证据不得进入仓库。 |

公开 SDK 和本地 mock 只能作为已有前置证据；它们不补齐上表中的真实客户端/Provider 输入。

## 4. 工作项

| ID | 工作项 | 交付物 | 对应验收 |
|---|---|---|---|
| C0-W1 | 收敛需求、阶段契约与计划边界 | `requirements/README.md`、阶段状态、单阶段计划规则 | C0 范围可判定 |
| C0-W2 | 固定 Codex baseline | 版本化 `environment.md`、无 secret 配置、transport 诊断 | C0-01、C0-02 |
| C0-W3 | 固定 Hermes baseline | 版本化 `environment.md`、无 secret Chat 配置 | C0-03 |
| C0-W4 | 定义 corpus 结构与脱敏规则 | fixture README、目录约定、重跑方式 | C0-04、C0-05、C0-06 |
| C0-W5 | 收集两客户端最小 wire corpus | 脱敏 request、upstream JSON/SSE、客户端观察和预期 terminal | C0-04、C0-05 |
| C0-W6 | 完成 C0 gate review | C0-01 至 C0-06 的证据链接和 Accepted/Blocked/Inconclusive 结论 | C0 退出条件 |

当前文档重构只完成 C0-W1；它不代表 C0-W2 至 C0-W6 已完成。

## 5. Corpus 最小要求

每个适用 case 至少保存：

- 固定客户端/Provider/模型版本；
- 无 secret 配置；
- 脱敏下游 request；
- 脱敏上游 JSON 或原始 SSE event bytes；
- 客户端实际观察；
- 预期 terminal 或错误分类；
- 重跑命令；
- “证明什么”和“不证明什么”。

最小场景：

| 场景 | Codex Responses | Hermes Chat |
|---|---|---|
| 文本 | stream 与 non-stream | stream 与 non-stream |
| 工具 | 单个与并行 call/output | 单个与并行 call/result |
| 流边界 | UTF-8、跨 chunk event、多 event、terminal | UTF-8、跨 chunk event、多 event、`[DONE]` |
| 异常 | provider error、EOF、partial stream、cancel、unknown event/field | provider error、EOF、partial stream、cancel、unknown field |
| 状态 | `previous_response_id` 原生样本 | 客户端实际支持的原生状态样本 |

这里定义的是 corpus 输入，不对代理实现是否通过作出 C1 结论。

## 6. 停止与变更规则

出现以下情况时停止扩展当前计划，先更新需求或记录 `Blocked/Inconclusive`：

- 固定 Codex 版本无法关闭 WebSocket 或不再支持目标 custom Provider profile；
- 真实客户端/Provider 无法在不泄露 secret 的条件下形成可复现 corpus；
- 目标 wire contract 与核心产品范围冲突；
- 新发现要求新增 Provider、协议、资源 API、工具执行或状态 ledger；
- 某项工作不能映射到 C0-01 至 C0-06 或 C0 退出条件。

发现项按[需求索引与阶段治理](../requirements/README.md)分类。它们不得直接变成新的 phase 或追加为 C1+ 工作包。

## 7. 验证与关闭

C0 关闭前只验证文档和 corpus 完整性，不执行代码阶段的质量门来替代 C0 证据：

1. C0-01 至 C0-06 均有证据链接和明确结论；
2. 两个目标客户端的版本、配置和重跑方式已固定；
3. corpus 不包含 credential、cookie 或私人 prompt；
4. 每项证据区分 mock/SDK、真实客户端和真实 Provider；
5. 核心需求、C0 阶段契约和当前实现说明不存在状态冲突；
6. C0 review 明确哪些结论进入 C1，哪些保持 `Blocked/Inconclusive`。

全部退出条件满足后：

- 将 C0 更新为 `Accepted`；
- 根据已接受的 C0 输入判断 C1 是否 `Ready`；
- 用新的 C1 单阶段计划替换本文；
- 不在本文末尾追加 C1 工作包。

## 8. 关联文档

- [需求索引与阶段治理](../requirements/README.md)
- [核心需求](../requirements/proxy-requirements.md)
- [阶段交付与研究需求](../requirements/delivery-requirements.md)
- [C0 阶段契约](../phases/00-scope-and-client-contracts.md)
- [目标客户端契约](../design/target-client-contracts.md)
- [当前实现说明](../implementation/current-implementation.md)
- [实验记录规范](../experiments/README.md)
