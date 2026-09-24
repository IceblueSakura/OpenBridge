# 旧运行时归档

旧路线的固定恢复点：`4f13ecefa21265a6ec5aa967278e81f03039f585`。`semantic-v1` 也保留前代实现，但其内容不与本恢复点完全相同。

[浏览固定源码树](https://github.com/IceblueSakura/OpenBridge/tree/4f13ecefa21265a6ec5aa967278e81f03039f585)。本地恢复依赖该 Git 对象，不依赖分支名指向或远端服务可用性。

## 范围与含义

| 归档范围 | 不再由当前工作区提供 |
|---|---|
| `src/main.rs`、`src/bin/`、旧 `ingress` / `pipeline` / `ir` / `bridge` | 旧 service/auth/probe 入口、Native/Bridge 请求与响应链路 |
| 旧 Provider、registry、models、credential/OAuth、observability、MCP、execution | 多 Provider 服务、登录、路由、监控和 MCP；含 test-only web_search executor 和未接线 ToolPlan |
| 旧专属 `tests/`、`testdata/`、`tools/corpus/`、`tools/probe/` | 旧运行时测试与语料工具，不作为 v2 验收门槛 |
| `config/` 的受版本管理模板、`docs/openapi.yaml` / `swagger-ui.html` | 旧配置形状、HTTP schema 和运行时 UI 资产 |
| 旧 `docs/decisions/`、`functional-requirements/`、运行指南和实现清单 | 旧路线设计/需求/能力陈述，不自动成为 v2 当前承诺 |

归档仅覆盖受版本管理的旧路线，**不表示 v2 功能对等或生产迁移成功**。私有配置、未跟踪内容与生成缓存不属于归档恢复合同。

参考资料和独立历史证据仍保留在 `references/`、`implementation-status/evidence/`；其中旧源码/合同链接指向固定 Git 版本，证据日期与适用范围不刷新。

## 只读查看

```sh
git show 4f13ecefa21265a6ec5aa967278e81f03039f585:src/execution/gateway_web_search.rs
git ls-tree -r --name-only 4f13ecefa21265a6ec5aa967278e81f03039f585 -- src tests config docs testdata tools
```

只读取受版本管理的旧文件，不读取本机私有配置。需要实际恢复时，在另行选定的目录或工作树操作；不要覆盖当前 v2 未提交工作。不在工作区保留第二份 `legacy/` 源码或兼容转发层。
