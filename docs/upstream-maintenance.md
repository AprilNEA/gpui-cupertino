# GPUI Alloy 维护入口

GPUI 自维护规范和补丁台账已迁移到独立仓库 [AprilNEA/gpui-alloy](https://github.com/AprilNEA/gpui-alloy)，唯一维护文件为 `main` 分支的 `ALLOY.md`。本仓库保留 Cupertino 组件、vendor 导入记录和消费方验收证据，不再复制补丁状态。

初始化文档目前已在同级本地仓库 `../gpui-alloy/ALLOY.md` 建立，尚待首次推送；推送后可查看 [在线维护规范](https://github.com/AprilNEA/gpui-alloy/blob/main/ALLOY.md)。已有上游 PR 继续保留在 [AprilNEA/zed](https://github.com/AprilNEA/zed)。

## 当前消费状态

- 上游基线 `U`：`4c841aaf1c4fa613e89a5d77096523d0ff593b56`，见 [vendor 来源说明](../vendor/README.md)。
- 当前仍使用本仓库的 `vendor/`，依赖来源没有切换到 Alloy。
- 9 项 vendor 补丁尚未迁入 Alloy。已有 2 个上游 PR 也尚未进入当前 vendor。
- Alloy 已验证快照 `D`、可消费 tag 和导入提交 `V`：待建立。候选导出和来源检查工具已完成，使用方法与验收边界见 Alloy 的 `ALLOY.md`；当前 vendor 尚未由 Alloy 快照重建。
- 原规范建立时的 `devenv test` 通过，只代表当时含未提交修改的 Cupertino 工作区；不代表 Alloy 或上游 PR 已通过验收。

## 后续导入记录

首次采用 Alloy 前，必须完成 `ALLOY.md` 中的迁移和两层验收。每次导入在本仓库记录来源仓库、`U`、`D`、快照 tag、完整路径清单、导出转换、检查结果及平台限制。导入后从 Git 历史取得 `V`，再追加记录。

导入提交必须同时包含 vendor、manifest、lockfile 和必要的 Cupertino 适配。首个可消费快照尚未建立，本次初始化不改变现有 vendor 实现。
