# OpenSekai-ASMR

OpenSekai-ASMR 是基于 [cubeww/OpenSekai](https://github.com/cubeww/OpenSekai) 的 ASM Unity 编辑器与原生 Rust 运行时迁移项目。Unity 源码、资源和项目设置保留为行为与视觉参照；Rust 运行时负责逐步接管谱面管理、编辑、演奏和结算流程。迁移的基准、运行方式和验收范围见 [native/README.md](native/README.md)。

> [!WARNING]
> Rust 运行时仍在迁移中，尚未覆盖全部应用入口，也没有证明与 Unity 画面逐像素一致。完整的已接通功能和未完成项目以 [native/README.md](native/README.md) 为准。

项目演示视频：[哔哩哔哩](https://www.bilibili.com/video/BV1xVah62E1c/)

## 当前内容

- Unity 工程仍提供原有的谱面编辑、第三方歌曲包和 Live 测试流程。
- Rust 运行时已经接入谱面读取与校验、SUS 导入、曲库扫描、手动／自动演奏、判定计分、音乐与演奏音效、结算界面，以及原生谱面编辑器的主要流程。
- `resources/opensekai` 保存固定版本的 OpenSekai 美术、音频和 Unity 资源快照；`resources/audio` 与 `resources/references` 保存独立的音频处理结果和视觉验收资料。

## 快速开始

Unity 工程使用 Unity `2022.3.62f3`，渲染管线为 URP 14。用 Unity Hub 打开本目录即可继续使用原编辑器。

构建并运行 Rust 命令行工具：

```sh
cargo build --release
cargo run --release -- inspect /path/to/score.json
cargo run --release -- audit-library /path/to/library
```

原生窗口、ASM 宿主、资源导入和回归检查命令集中记录在 [native/README.md](native/README.md)。

## 目录

- `Assets/`、`Packages/`、`ProjectSettings/`：原 Unity 工程和迁移参照资源。
- `native/`：Rust 运行时、C ABI、基准数据和迁移检查工具。
- `resources/opensekai/`：固定提交的 OpenSekai 资源快照、清单和音效绑定表。
- `resources/audio/`：用户音频的无损处理结果及校验信息。
- `resources/references/`：用于视觉对照的用户录屏、封面和切片。
- `resources/generated/`：迁移过程中生成并单独记录来源的视觉资源。
- `tools/`：资源导入、转换和验证脚本。
- `content/`：本地谱面包和官谱整理目录，属于本机数据，不作为源代码发布内容。

## 许可与资源

本仓库中的代码以 MIT 协议发布。第三方库、字体、音频、贴图、Prefab、Shader，以及来自原作或其他权利方的资源不自动包含在 MIT 授权范围内；请遵循各自的原始许可和权利归属。资源来源和固定版本见 [resources/opensekai/README.md](resources/opensekai/README.md)。

本项目与 Project Sekai、其发行方或其他权利方没有官方关联。
