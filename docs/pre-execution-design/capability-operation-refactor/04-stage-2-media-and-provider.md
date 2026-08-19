# 04：阶段 2——Media profiles 与 Provider catalog

## 目标

建立 image/audio/file 可长期扩展的 typed capability layer，并让每个 Target 显式选择完整 executable media profile，消除 Provider ceiling 自动继承和 registration 手工清除字段的风险。

## 依赖

- 阶段 1 的 operation/task binding 已落地；
- 当前 image、audio、file 需求与 status 重新核对；
- 当前 Provider media constants 和 Target mutation 路径已盘点；
- 本阶段不改变标准下游 wire。

## Direct replacement

1. 将 generation capability 按责任拆为 facade、tools、structured output、reasoning、media 子模块。
2. 引入 Chat/Responses operation-specific media envelope。
3. 保留 image source-owned discriminated union；将 audio source 改为各 source 自己拥有 formats/limits payload。
4. 用 `ChatFileInputProfile` / `ResponsesFileInputProfile` 替换 `file_input: bool` 和 reserved true assertion。
5. 将每个 Provider 的 media ceiling 与具名 Target profiles 集中到 `providers/<provider>/media.rs`。
6. `to_executable()` 要求完整 media profile；默认 profile 为全关闭，不再继承 ceiling。
7. Registry validation 对每个媒体 profile 执行 validate、subset 和组合可达性检查。
8. Bridge contribution 对所有 media 使用明确空 profile；不能在多个字段中分别清除。

## Profile 原则

- source 缺失用 `None` 或 union variant 表示，不使用零 limit 哨兵；
- part、URL、encoded、decoded、per-item、total 使用有单位语义的字段或 newtype；
- source/format/detail 集合非空、唯一；default 与 allowed domain 分开；
- 交集完成后必须重新验证 aggregate reachability；
- 跨 Provider 不共享事实常量；同 Provider family 内可复用已证明的具名 profile；
- `file_id` / image `file_id` 在 resource affinity 完整前不进入 executable source set。

## 先失败测试

- Provider ceiling 新增 image/file/audio 时，未显式选择的兄弟 Target 不提升；
- executable profile 超过 Provider ceiling 在启动前失败；
- source payload 不完整、limit 为零、total 不可达、format 重复时失败；
- audio 不再通过 zero sentinel 表示 source absence；
- file bool 已不存在，typed file profile 可以参与 subset/intersection，但生产 Target 仍可全部关闭；
- Bridge candidate 使 media intersection 按合同关闭，不按请求绕过；
- profile intersection 满足 idempotence、commutativity 和 candidate-order independence。

## 实施步骤

1. 机械拆分 core media modules，保留行为；
2. 引入 common limits/source primitives；
3. 迁移 image；
4. 迁移 audio source payload 与 audio task profiles；
5. 引入 typed file profile 但不开放生产能力；
6. 迁移 Provider-local media constants；
7. 修改 Target registration 为显式完整 profile；
8. 修改 contribution/aggregate 适配新 envelope；
9. 删除旧 fields、mutation helpers、zero sentinels 和 reserved bool。

## 删除清单

- `file_input: bool`；
- audio source absence 的零值语义；
- registration 中 `capabilities.image_input = None` 等媒体突变；
- 跨 Provider 公共 profile fact；
- 平铺 `image_input/audio/file_input` contribution fields；
- 迁移 alias 或旧/new profile conversion。

## 退出门

- 现有 MiMo、ChatGPT、OpenRouter、Kimi、NVIDIA 等媒体 Models/preflight/wire 行为保持；
- Provider 扩展不能隐式提升 Target；
- 纯 profile algebra、registry contract、compiled Models 和 zero-egress tests 全绿；
- 完整 Rust 基线通过。

## 非目标

- 不开放生产 file input；
- 不实现 Files lifecycle、resource ledger、OCR 或 media transform；
- 不发布 Models v2；
- 不新增 Images/Audio 专用 endpoint。
