# GPUI Alloy 维护入口

GPUI 自维护规范和补丁台账位于 [AprilNEA/gpui-alloy](https://github.com/AprilNEA/gpui-alloy) 的 `main` 分支：[ALLOY.md](https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md)。本仓库记录 Cupertino 消费状态。每个上游候选在 Alloy 中保留独立主题分支；已有 PR 继续使用 [AprilNEA/zed](https://github.com/AprilNEA/zed)。

## 当前消费状态

2026-10-03：首次从 Alloy 的已验收固定快照重建 `vendor/`，纳入原有 9 项补丁及已有 2 个上游 PR 的本地适配。后续 GPUI 修改必须先进入 Alloy 的独立主题分支，再经过集成、导出和消费方验收。

- 上游基线 `U`：`4c841aaf1c4fa613e89a5d77096523d0ff593b56`。
- Alloy 快照 `D`：`19fc9968f689ccce2b9e31c2eb9a76ec442333dc`。
- 快照 tag：`gpui-alloy/20261003.1`，附注 tag，暂不签名。
- 导出记录：[vendor/ALLOY-SNAPSHOT.json](../vendor/ALLOY-SNAPSHOT.json)。记录含完整路径、文件摘要、包路径、U、D 和 exporter 摘要。
- exporter SHA-256：`fa10e78736b16d03c92c6740850a5845c18ffc21b101b61cb885da370831c000`。
- 导出记录 SHA-256：`465379119dbb45efd5dfd5335675a3ed6003337a1397f6dd091c0d6f96e58f63`。
- `Cargo.lock` SHA-256：`9ae61f398a36a8f1f00329e7fc26abf9913ac797b2750437981e8545dbe7fef2`，本次未改动。
- 导入提交 `V`：从包含本记录的导入提交取得；完整 SHA 和验收结果记入 Alloy 主分支的后续事件，避免提交自引用。

导出保留原 crate 名称、完整 crate 目录、字体和许可证。根 workspace 按普通、target、optional、build、dev 的 path 依赖闭包生成，共 26 个 crate、436 个清单文件。许可证按上游恢复为指向保留的根许可证的符号链接。原始根构建控制文件保存在 `vendor/alloy-source/` 供追溯；消费方不自动启用这些配置。消费方自己的 manifest、工具链和 lockfile 控制外部依赖。

隔离消费方的来源检查已确认 18 个实际使用的配套 crate 全部来自该快照，使用 `aarch64-apple-darwin`、全部消费方 features 和 `--locked`。格式、Clippy 和 119 项测试通过；该验证包含当时保留的组件暂存工作，不能冒充干净 V 的检查。原工作区导入后的 `devenv test` 也已通过；V 的干净工作区检查结果另记入 Alloy 主台账。

Alloy 的固定 D 已通过完整组合验收：491 项库测试、5 项 scene、17 项 Metal、popup 示例和两项原生窗口 harness；另在预编译 Metal 路径执行 3 项 continuous corner 检查。全部 features 库测试使用 `--test-threads=1`；默认并行模式有 4 个既有 profiler 全局状态隔离失败，源码与 U 相同，失败日志保留，未修改断言或宣称并行检查通过。

验证环境为 macOS `26.4 (25E246)`、Apple M5 Max、Xcode `26.6 (17F113)`、Rust `1.98.1`、Cargo `1.98.1`。当前两个显示器均为 `2×`，不代表已验证跨显示器迁移。材质仍限原生 macOS Metal 的不透明 SDR 窗口；Inactive Clear 要求 Apple GPU、Metal 3.1 和可参数化的 RGB ColorSync 配置。HDR/EDR、混合 DPI、原生 VoiceOver/IME 及完整历史原生截图采样未验收。

Alloy 分支、文档和 tag 当前仅在本地，尚未推送。未发布 crate、未新建 PR，原有 PR 分支未改写。由维护人按 Alloy 主台账的明确引用列表推送。

## 重现与回退

在 Alloy 仓库使用上述 U、D 和 exporter 导出到新目录。先运行 `verify-consumer`，再执行本仓库的 `devenv test`。必须在 vendor 之外保存构建产物，保持来源清单可核对。

本次导入前的 vendor 来源已固定在 Cupertino 提交 `c79bb253dd456877b61e1503ec4f2c549838a86e`。发生回归时，用新的提交恢复该来源的整套 vendor 及与其配套的消费方配置；不要移动快照 tag 或撤销无关组件工作。本次未改变 manifest 和 lockfile。旧构建缓存不属于源码或回退记录。
