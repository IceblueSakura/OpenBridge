# 旧版本历史参考

当前 `main` 以原 `semantic-v2` 实现为基线，旧版本不再定义主线目标、兼容义务或验收门槛；仅保留 Git refs 与必要参考。既有 `semantic-v1`、`v0.1` 分支保留其各自版本，不表示需要合并或恢复它们。

旧路线的固定参考点：`4f13ecefa21265a6ec5aa967278e81f03039f585`。`semantic-v1` 也保留前代实现，但其内容不与本参考点完全相同。

[浏览固定源码树](https://github.com/IceblueSakura/OpenBridge/tree/4f13ecefa21265a6ec5aa967278e81f03039f585)。本地查阅依赖该 Git 对象，不依赖分支名指向或远端服务可用性。

## 范围与含义

| 归档范围 | 不再由当前工作区提供 |
|---|---|
| `src/main.rs`、`src/bin/`、旧 `ingress` / `pipeline` / `ir` / `bridge` | 旧 service/auth/probe 入口、Native/Bridge 请求与响应链路 |
| 旧 Provider、registry、models、credential/OAuth、observability、MCP、execution | 多 Provider 服务、登录、路由、监控和 MCP；含 test-only web_search executor 和未接线 ToolPlan |
| 旧专属 `tests/`、`testdata/`、`tools/corpus/`、`tools/probe/` | 旧运行时测试与语料工具，不作为 v2 验收门槛 |
| `config/` 的受版本管理模板、`docs/openapi.yaml` / `swagger-ui.html` | 旧配置形状、HTTP schema 和运行时 UI 资产 |
| 旧 `docs/decisions/`、`functional-requirements/`、运行指南和实现清单 | 旧路线设计/需求/能力陈述，不自动成为 v2 当前承诺 |

历史参考仅覆盖受版本管理的旧路线；**不要求主线功能对等，也不证明当前实现生产就绪**。私有配置、未跟踪内容与生成缓存不属于此参考范围。

工作区不保留历史分析、审计或测试报告。[协议来源](references/README.md)仅保留仍使用的标准出处、版本和许可；历史问题可显式查询 Git，但不能用旧记录代替当前绑定、行为或可用性查询。

## 只读查看

```sh
git show 4f13ecefa21265a6ec5aa967278e81f03039f585:src/execution/gateway_web_search.rs
git ls-tree -r --name-only 4f13ecefa21265a6ec5aa967278e81f03039f585 -- src tests config docs testdata tools
```

只读取受版本管理的旧文件，不读取本机私有配置。需要实际恢复时，在另行选定的目录或工作树操作；不要覆盖当前 v2 未提交工作。不在工作区保留第二份 `legacy/` 源码或兼容转发层。
