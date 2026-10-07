### Ojsk Community

本地 ASM 编辑器的 Rust 迁移入口见 [native/README.md](native/README.md)。原 Unity 源码与资源保留为对照基准，迁移尚在进行。

 <img src="https://www.jsoftstudio.top/css/Jsoft_logo.png" width = "100" height = "100" alt="Jsoft_logo" align=center />

###### ©2024-2026 Jsoft Studio

------

<img src="https://img.shields.io/github/stars/kamcdev/OpenSekai_Community.svg">

<img src="https://img.shields.io/badge/交流QQ群-984242265-purple">

<img src="https://img.shields.io/badge/OpenSekai官方群-787994708-purple">

<img src="https://img.shields.io/badge/B站-J软件官方-light">

<img src="https://img.shields.io/badge/官网-www.jsoftstudio.top-yellow">

------


> [!CAUTION]
> 此仓库已进行归档并被放弃维护，未来大概率不会继续更新

> [!tip]
> 欢迎clone本仓库，基于功能丰富的社区版继续开发

Ojsk Community 是一个用于学习和研究目的的 Project Sekai 音乐游戏玩法复刻项目，目前提供可用的**谱面编辑器**、**第三方歌曲包支持**，以及**从编辑器进入 live 测试游玩**的完整流程。

## 开发环境

- Unity: 2022.3.62f3
- 渲染管线: URP 14
- 主要依赖: TextMesh Pro、UniTask、MessagePack、uPalette、UI Particle、SoftMaskForUGUI

## 目录说明

- `Assets/Scripts/Assembly-CSharp/Sekai`：主要游戏逻辑。
- `Assets/Resources`：运行时通过 `Resources` 加载的 Prefab、文本、字体、特效和界面数据。
- `Assets/Sekai/assetbundle/resources`：原样放置的 AssetBundle 源资源，资源本身应保留 AssetBundle 名称，之后通过构建流程打包。
- `Assets/StreamingAssets`：打包后的 AssetBundle 与本地运行数据输出位置，主要用于构建后的运行环境。
- `Assets/Editor/OjskCommunityAssetBundleBuildPipeline.cs`：本地 AssetBundle 构建流程。

## 资源说明

本仓库中的代码以 MIT 协议发布。第三方库、字体、音频、贴图、prefab、shader 参考资源以及任何可能来源于原作或其他权利方的资源，不自动包含在 MIT 授权范围内，请分别遵循其原始许可和权利归属。

本项目与任何原作游戏、发行方或权利方没有官方关联。
