# GPUI Alloy 维护入口

GPUI 自维护规范和补丁台账位于 [AprilNEA/gpui-alloy](https://github.com/AprilNEA/gpui-alloy) 的 `main` 分支：[ALLOY.md](https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md)。精简 Alloy 保存独立 GPUI 源码、维护文档和导出工具。本仓库记录 Cupertino 消费状态。

完整 Zed 来源、集成提交 D 和各上游候选的独立主题分支位于 [AprilNEA/zed](https://github.com/AprilNEA/zed)；已有 PR 继续使用原分支。精简前的 Alloy 完整历史、旧 D 和旧快照 tag 保存在 [AprilNEA/gpui-alloy-archive](https://github.com/AprilNEA/gpui-alloy-archive)。后续生产源码修改先在完整来源仓库形成独立主题和集成 D，再更新精简 Alloy 的来源记录、固定 S 并完成消费方验收。

## 当前消费状态

2026-10-03：本次消费来源改为精简 Alloy 的独立提交 S，快照 tag 为 `gpui-alloy/20261003.2`。原有 9 项补丁及已有 2 个上游 PR 的本地适配继续由完整源 D 提供。

- 上游基线 `U`：`4c841aaf1c4fa613e89a5d77096523d0ff593b56`。
- 完整源集成提交 `D`：`d21987f81a013ec67945892506bcc6aae8a2db0f`，保存在 `AprilNEA/zed` 和旧 Alloy archive。
- 精简 Alloy 提交 `S`：`9d59ea617d75d02e4645eefd22844235431138c8`。
- 当前快照 tag：`gpui-alloy/20261003.2`。
- 消费导出记录：[vendor/ALLOY-SNAPSHOT.json](../vendor/ALLOY-SNAPSHOT.json)，格式为 2。`revision` 表示 S，`source.revision` 表示 D，`source.upstream` 表示 U。
- 独立来源合同：[vendor/ALLOY-SOURCE.json](../vendor/ALLOY-SOURCE.json)。
- 原格式 1 导出记录：[vendor/alloy-source/ALLOY-SNAPSHOT.json](../vendor/alloy-source/ALLOY-SNAPSHOT.json)，保留原始字节；SHA-256 为 `88a1781f7241d1f6a5f0f178a63230e0730085fce93c0f1049b66445a076615a`。
- 本次 exporter SHA-256：`9a82458aa24bedaaf9f592f57d2f956e612dfab1dc31265190e7624bebb69d45`。
- 本次格式 2 导出记录 SHA-256：`e1cf291d98001e41b5cac87c8d81f832ac533b504a0cf33ca9b32265764ce34b`。
- 消费方 `Cargo.lock` SHA-256：`9ae61f398a36a8f1f00329e7fc26abf9913ac797b2750437981e8545dbe7fef2`，本次不改动。
- 导入提交 `V`：从包含本记录的导入提交取得；完整 SHA 和验收结果记入 Alloy 主分支的后续事件，避免提交自引用。

来源链为 U→D→S→V。只有 U→D 是 Git 祖先关系；D→S 是固定来源的提取关系，S→V 是消费导入关系。S 属于独立 Git 历史，不包含 U 或 D 这两个提交，也不要求 U 是 S 的祖先。

导出保留原 crate 名称、完整 crate 目录、字体、许可证、符号链接和执行位。根 workspace 按普通、target、optional、build、dev 的 path 依赖闭包生成，包含 26 个 crate、438 个清单文件。生产源码保持 D 的原始导出内容；新记录额外保留来源合同和原格式 1 记录。

原始根 `Cargo.toml`、`Cargo.lock`、`.cargo/config.toml` 和 `rust-toolchain.toml` 继续保存在 `vendor/alloy-source/` 供追溯。消费方不自动启用这些配置，也不继承精简 Alloy 根目录的 patch、profile、工具链或 lockfile。本次不修改消费方根 manifest、crate manifest、lockfile 或 devenv 配置；现有 path 依赖继续指向 `vendor/crates/`。

本次验收结果：

精简工作区的格式、Clippy、517 项测试及两项原生窗口 harness 已通过；Python 导出工具的 Ruff 与 4 项回归测试通过。Cupertino 的来源检查确认 18 个可达配套 crate 均来自 S，当前工作区的 `devenv test` 通过。该工作区包含原有组件暂存工作；干净导入提交 V 的单独验收结果在提交完成后记入 Alloy 主台账。

来源检查、生产文件投影检查与行为验收分别记录。原有 D 的通过结果不能代替本次 S 和 V 的验收。含组件暂存工作的工作区检查与干净 V 检查也必须区分；无关组件工作不属于本次导入提交。

材质仍限原生 macOS Metal 的不透明 SDR 窗口；Inactive Clear 要求 Apple GPU、Metal 3.1 和可参数化的 RGB ColorSync 配置。HDR/EDR、混合 DPI、原生 VoiceOver/IME 及完整历史原生截图采样未验收，不能因仓库精简而扩大支持声明。

## 重现与回退

使用本记录对应的 exporter 版本，从固定 S 导出到一个尚不存在的新目录。新版 CLI 不接受旧的 `--upstream` 参数；U 和 D 从 S 中的来源合同读取。

```sh
ALLOY_REPO=/Users/Xuan/Developer/AprilNEA/gpui-alloy
ALLOY_S=9d59ea617d75d02e4645eefd22844235431138c8
ALLOY_CONSUMER=/Users/Xuan/Developer/AprilNEA/gpui-cupertino
python3 "$ALLOY_REPO/script/gpui_snapshot.py" export --repo "$ALLOY_REPO" --revision "$ALLOY_S" --snapshot /tmp/gpui-alloy-20261003.2
```

核对新目录的完整差异并导入 `vendor/` 后，在 Cupertino 根目录执行来源与行为检查：

```sh
cd /Users/Xuan/Developer/AprilNEA/gpui-cupertino
devenv shell -- python3 "$ALLOY_REPO/script/gpui_snapshot.py" verify-consumer --repo "$ALLOY_REPO" --revision "$ALLOY_S" --snapshot "$ALLOY_CONSUMER/vendor" --consumer "$ALLOY_CONSUMER" --target aarch64-apple-darwin
devenv test
```

`verify-consumer` 从固定 S 重新导出并核对完整文件清单和记录，然后即时检查全部消费方 features、`--locked` 和 `aarch64-apple-darwin` 下的依赖解析。每个实际使用的配套包必须来自对应 vendor 路径，不能混入旧 vendor、registry 或另一份 Git 来源。必须在 vendor 之外保存构建产物。

本次导入的直接回退点为 Cupertino 提交 `ea9cc093403f45a26f32d0ae415007fef95e3b09`。发生回归时，用新的提交恢复该提交中的整套 vendor 和对应维护记录；不要移动快照 tag 或撤销无关组件工作。本次 manifest 和 lockfile 不变。旧构建缓存不属于源码或回退记录。

重现旧 D 的格式 1 导出时，使用 `gpui-alloy-archive` 中保留的旧 exporter、U 和 D。不要用新版 S 导出入口解释旧记录，也不要将 archive 的 `gpui-alloy/20261003.1` 当成本次精简仓库的快照。

## 首次完整来源导入记录

以下记录保留精简前的历史状态，不表示本次 S 已通过相同检查。

2026-10-03：首次从原 Alloy 的已验收固定快照重建 `vendor/`，纳入原有 9 项补丁及已有 2 个上游 PR 的本地适配。当时的完整来源和独立主题分支保存在原 Alloy；该仓库现为 `gpui-alloy-archive`。

签名后的 D 为 `d21987f81a013ec67945892506bcc6aae8a2db0f`。旧快照 tag 为 `gpui-alloy/20261003.1`，是使用 1Password SSH 密钥签名的附注 tag，现保存在 archive。旧 exporter SHA-256 为 `fa10e78736b16d03c92c6740850a5845c18ffc21b101b61cb885da370831c000`；旧格式 1 记录 SHA-256 为 `88a1781f7241d1f6a5f0f178a63230e0730085fce93c0f1049b66445a076615a`。旧导出包含 26 个 crate、436 个清单文件。

当时隔离消费方的来源检查确认 18 个实际使用的配套 crate 全部来自该快照，使用 `aarch64-apple-darwin`、全部消费方 features 和 `--locked`。格式、Clippy 和 119 项测试通过；该验证包含当时保留的组件暂存工作，不能冒充干净 V 的检查。原工作区导入后的 `devenv test` 也已通过；干净 V 的检查结果另记入原 Alloy 主台账，随 archive 保留。

固定 D 的完整组合验收为 491 项库测试、5 项 scene、17 项 Metal、popup 示例和两项原生窗口 harness；另在预编译 Metal 路径执行 3 项 continuous corner 检查。全部 features 库测试使用 `--test-threads=1`；默认并行模式有 4 个既有 profiler 全局状态隔离失败，源码与 U 相同，失败日志保留，未修改断言或宣称并行检查通过。

当时验证环境为 macOS `26.4 (25E246)`、Apple M5 Max、Xcode `26.6 (17F113)`、Rust `1.98.1`、Cargo `1.98.1`。两个显示器均为 `2×`，不代表已验证跨显示器迁移。原生与材质支持边界仍按各次实际验收记录解释。

首次导入前的 vendor 来源固定在 Cupertino 提交 `c79bb253dd456877b61e1503ec4f2c549838a86e`；后续签名版本见下一节。该提交是首次迁移的历史来源，不是本次格式 2 导入的直接回退点。

## 首次发布前签名迁移

2026-10-03：维护人明确要求重新签名后推送。原 Alloy 的签名 D 为 `d21987f81a013ec67945892506bcc6aae8a2db0f`，与原 D `19fc9968f689ccce2b9e31c2eb9a76ec442333dc` 的完整源码树一致。旧 D、旧导入提交和原 tag 对象均由签名归档保留，原 Alloy 侧的历史和旧 tag 现位于 `gpui-alloy-archive`。当时同名 tag 重建仅适用于首次远端发布前的明确授权，不改变后续快照不可变规则。

冻结提交的签名版本为 `7bb5515bae5ab84db2360324a1bc8fc12ae6f1b4`；原导入提交的签名版本为 `a316fcf8ea2f9f4877f4f197bc6044b60619ffb9`。二者的源码树、作者、消息和父拓扑均已核对。随后从签名 D 重新导出，仅 `vendor/README.md` 与 `vendor/ALLOY-SNAPSHOT.json` 的来源信息发生变化；生产源码、资源、manifest 和 lockfile 均保持原样。来源检查与签名后验收日志由原 Alloy 主台账记录，现随 archive 保留。

维护人当时授权首次远端发布前的重新签名与推送。提交、tag 的旧→新映射及远端核对结果继续保留在 archive 的台账中。原有 Zed PR 分支保持不变；本次精简使用新的 `gpui-alloy/20261003.2`，不改写旧快照 tag。
