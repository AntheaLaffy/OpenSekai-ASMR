# OpenSekai 美术与音频资源

这批资源来自用户克隆的 `RERASER/OpenSekai`，固定于提交
[`17a62c4a306fc753ef0f8edfc64867182c936920`](https://github.com/RERASER/OpenSekai/tree/17a62c4a306fc753ef0f8edfc64867182c936920)。
`upstream/` 内保存该提交的原始字节、目录名和 Unity `.meta`，全部使用普通 Git 文件追踪；没有指向旁边仓库的链接，也不需要 Git LFS 或原克隆目录才能使用。

两个 Unity 项目的资源路径、部分内容和 GUID 不同，因此资源放在独立目录。当前 `Assets/` 的 6,560 个基准文件保持原样，编辑器仍默认使用 LXGW WenKai，保留中文和英文界面。这里的字体供迁移对照，不会自动替换编辑器字体。

## 收录内容

原始快照共 **1,723 个文件，140.51 MiB**，其中包括：

| 内容 | 数量 |
| --- | ---: |
| Tell Your World 音乐 | 1 个 WAV，132.2 秒，44.1 kHz、16-bit、双声道 |
| Live 默认音效 | 28 个 WAV |
| Tap custom01 音效 | 14 个 WAV |
| Tap custom02 音效 | 20 个 WAV |
| 贴图与封面 | 154 个 PNG |
| Prefab | 121 |
| 动画 / 动画控制器 | 14 / 14 |
| 材质 / Shader / Shader Graph | 71 / 32 / 4 |
| 字体文件 | 3 个 OTF、1 个 TTF，另含字体图集与材质 |
| Tell Your World 原始 SUS 谱面 | EASY、NORMAL、HARD、EXPERT、MASTER、APPEND |

同时保留场景、渲染资源配置、Shader include、其他 Unity 资源和 907 个 `.meta`。不导入旧 C# 运行时、插件 DLL、编辑器缓存或项目设置。Prefab 和场景是迁移参照；把资源收录进仓库不代表它们对应的动画、粒子和音效触发逻辑已经移植到 Rust。

- [manifest.json](manifest.json)：原提交、原路径、Git blob、SHA-256、字节数、资源 GUID，以及与现有资源完全相同的文件位置。
- [audio-catalog.json](audio-catalog.json)：全部 63 个 WAV 的目录分组、声道、采样率、时长和可确定的 cue 名。`acb_…cue…wav` 保留原名，未猜测对应事件。
- [soundbank.json](soundbank.json)：从 `FrontUIView.prefab` 的 GUID 引用解析出 16 个 cue、17 个片段，另按原 `clip_FINISH_v2_01.anim` 的 `PLaySE` 事件接入 `se_live_finish`，合计 17 个 cue、18 个片段；保留事件的来源哈希和时间点，运行时在倒计时前解码。
- [upstream/README.md](upstream/README.md) 与 [upstream/LICENSE](upstream/LICENSE)：来源说明与原许可。第三方媒体的权利归属沿用来源说明，不改写为 Rust 代码的 MIT 许可。

Tell Your World 音频的 Git blob 为 `c84a4b79df1787c4767517df5028066881548b47`，
SHA-256 为 `6de072a6be4d2b400da045d2d76e906a68da792fed70734da44797450e39478e`。
前置时间 **9 秒**来自这份快照 `Assets/Scenes/LiveScene.unity` 的 `previewFillerSec`，与现有官谱元数据相符。

## 校验与重复导入

在 `OpenSekai_Community_ASMR` 根目录执行：

```sh
python3 tools/import_opensekai_resources.py verify
# 额外确认全部资源已在 Git 索引中，并且索引 blob 与磁盘完全一致
python3 tools/import_opensekai_resources.py verify --git-index

# 仅在需要重建同一份快照时使用原克隆；已有不同文件会报错，不会覆盖
python3 tools/import_opensekai_resources.py import --source ../OpenSekai --revision 17a62c4a306fc753ef0f8edfc64867182c936920
```

导入器读取指定 Git 提交，忽略来源工作区的未提交改动；同时校验已有 Unity 基准。`.gitattributes` 禁用快照的换行转换，使 Git blob 在检出后仍可核对。

## 将正曲和封面接入曲库

现有 `0001_01` 六个难度包已附上相同的 `music.wav`、`jacket.png` 和媒体来源，保留各自的谱面与曲目元数据。复制出的可编辑曲目包仍位于本地 `content/library`；可重建的文件不重复纳入 Git，原始音乐和封面在本目录中追踪。

已有官谱时可以重复执行：

```sh
python3 tools/import_opensekai_resources.py attach-song
```

工具只匹配 musicId 1 / `0001_01` / 原快照 Tell Your World 的包；不同的用户音频或封面会阻止写入。使用普通文件复制，因此曲目包可以独立导出，不依赖源目录符号链接。

全新空曲库也能直接用本仓库收录的六张 SUS（需先构建 Rust CLI）：

```sh
target/release/opensekai import-sus resources/opensekai/upstream/Assets/Charts content/library
python3 tools/import_opensekai_resources.py attach-song
```

从 ASM 编辑器根目录打开当前官谱：

```sh
./build/editor --project application/OpenSekai_Community_ASMR --app-chart official-0001_01-easy --app-screen play
```

Rust 已接入这些 cue 的判定、空击、摩擦/划动、连接点、长条循环和手动结算音，ASM 宿主独立混音，不改变音乐时钟。音效选择有 112 组原 C# 执行对照；C ABI 检查同拍合音、轮换、暂停/恢复和重新演奏，实际窗口检查采集判定及循环音输出。角色语音、完整倒计时/技能演出及其他音效皮肤仍待迁移；素材收录和当前音效接通不等同于已完成所有演出效果。

绑定表可用 `python3 tools/compile_soundbank.py` 重建。音效选择基准可用 `python3 tools/capture_sound.py` 重新执行原 C# 核对；更新基准需要显式添加 `--capture`。

Life 归零仅影响显示和最后的结果类型，不会中断判定或音乐。最后一个音符处理完后，手动演奏按 AP / FC / CLEAR / FINISH 选择结算；使用原字形 Sprite、关键帧曲线、光环网格、加法混合和粒子配置。FINISH 音效在原动画的 0.06666667 秒事件触发，其他三种在演出开始时触发；动画等待期间音乐尾段继续播放。AP／FC／CLEAR 的结算粒子已接入，FINISH 保留原动画禁用粒子的状态。粒子的阻尼与随机分布仍需对照校准，角色语音和完整视觉对照仍待完成。

用户提供的《里表情人》视觉基准单独保存在 [resources/references/0114_01](../references/0114_01/README.md)。按可见状态比较截图，保留 Rust 自身的数据和逻辑。原版背景和封面多层显示已接入；确实缺失的图片可用 imagegen 补图，生成素材单独保存并标注来源。

## English

This directory preserves the selected art and audio from the pinned OpenSekai commit as ordinary Git-tracked files. It contains the original Tell Your World WAV, all 62 sound effects, cover art, six SUS charts, textures, prefabs, animation, shaders, fonts and Unity metadata. The original application baseline and the editor's Chinese/English UI with LXGW WenKai remain unchanged.

Run `python3 tools/import_opensekai_resources.py verify --git-index` to verify the snapshot and Git index. Run `attach-song` to copy the music and cover into existing Tell Your World packages without replacing different user media. The snapshot is independent of the source clone. The Rust runtime uses the prefab's 16 cues plus the FINISH animation event (18 clips total), with a separate host mixer. Zero Life does not interrupt play. Ending sprites, curves, ring mesh, additive materials, particle modules and timed SE are connected. Particle calibration, voices, additional sound skins and complete visual equivalence remain unfinished. User footage and generated art are kept separately in `resources/references` and `resources/generated`.
