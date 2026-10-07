# OpenSekai 的 Rust 迁移

目标是保留当前项目的视觉和功能，让应用在 ASM Unity 编辑器内运行。原始 `Assets`、`Packages`、`ProjectSettings` 保持原样，作为迁移参考；最终运行时不依赖 C# 或 Unity。

当前是迁移中的运行时，**尚未完成全部应用，也尚未证明 Unity 画面逐像素一致**。不能用管理页截图或数据测试代替整款应用的验收。

## 已建立的基准

- 项目提交 `0cd2e6212d71671d48373732fbaa3559c16a765a`，Ojsk Community 1.6.16，Unity 2022.3.62f3。
- `baseline/files.json` 固定 6,560 个原文件的大小和 SHA-256；默认分辨率 1920×1080。
- `baseline/manager-source.json` 从原管理页提取文案、字号、样式、色值和布局常量。
- `baseline/oracle.json` 执行原 C# 的 Manifest.Normalize、CreateStableMusicId、BPM 换算、格点量化与轨道宽度方法。随机 ID 提供器用固定值替代。时间换算按单精度位值比较；时间转 ticks 用 ties-to-even，格点量化用 AwayFromZero，二者不能混用。
- `baseline/minimap.json` 执行原 `MusicScoreMinimapView` 的显示范围、拍号、小节线、音符和连线像素循环。六组短谱、长谱、首尾边界案例与 Rust 逐像素比较。Unity 的数学/颜色值类型用小型替身承接，未模拟完整 Unity 渲染器。
- `baseline/long-notes.json` 执行原 `LongNoteLinePreview` 的直线、缓入/缓出网格方法，比较六组顶点、UV 和三角形对角线。ASM 应用 ABI 5 保留图集纹理四边形、按键释放、音乐样本时钟与资源准备握手，并增加独立音效音库和播放命令；独立检查缩放、裁剪和 GPU 顶点 UV 顺序。
- `baseline/sus.json` 执行原 `SUS/Converter.cs` 的读取、合并与覆盖方法，比较 4 组输入的 791 个合并节点。真实官谱另以用户提供的曲目表核对判定数，覆盖 3,502 张有计数的谱面；5 张教程谱没有对应计数。
- `baseline/sound.json` 执行原 `TapEffectView` 的音效表与判定选择方法，112 组情况与 Rust 对照。源 Prefab 的 GUID 绑定另生成 `resources/opensekai/soundbank.json`，包括关键摩擦音的两片段轮换；不按文件名猜测绑定。
- `baseline/project.json` 明确记录 `unity_raster_reference: null`。目前只有源码契约基准，没有冒充原 Unity 实机截图。

编辑器默认简体中文、LXGW WenKai，并支持英文。应用中文保持源项目文案，使用项目内原字体文件；英文为新增方案。文件虽名为 `FOT-RodinNTLGPro-EB/DB.otf`，本次检查到的内嵌字体家族是 Source Han Sans SC / Medium，迁移保留这些实际文件。应用字体和编辑器字体分别加载，避免编辑器的字体偏好改变游戏原有视觉。

## 运行与验证

在 ASM 编辑器根目录执行：

```sh
make opensekai-editor
make opensekai-check
make opensekai-assets
make opensekai-window-check
make opensekai-live-check
make opensekai-song-check
./build/editor --project application/OpenSekai_Community_ASMR --app-only --headless --size 1920 1080 --output build/opensekai-manager.png
./build/editor --project application/OpenSekai_Community_ASMR --data-dir /path/to/charts --app-screen maker
```

`--data-dir DIR` 指定独立谱面根目录，直接包含各曲目的文件夹。默认合并项目 `content/library` 与 `$XDG_DATA_HOME/opensekai-rs/CustomMusicScores`（或 `~/.local/share/opensekai-rs/CustomMusicScores`）。验证使用临时目录，不写入用户原有谱面。

## 本地官谱与演奏

原 `pjsk官谱合集` 已归档至 `content/official/source`，旧路径保留相对符号链接；8,366 个原文件的哈希记录在 `source-files.json`。3,507 个 SUS 难度文件转换为 `content/library/<歌曲ID>-<难度>/score.json` 与 `manifest.json`。`官谱id信息对照表.xlsx` 保持原样并复制归档，按 musicId 补入曲名、等级、作者、前置空白秒与官方判定数；教程不猜填。原 Unity 的 6,560 个参考文件不参与整理。

```sh
# 下列命令在 OpenSekai_Community_ASMR 目录执行；已有包保留用户编辑
python3 tools/organize_official.py
python3 tools/official_metadata.py
target/release/opensekai audit-library content/library
python3 tools/capture_sus.py
```

管理页搜索支持曲名、ID、难度、作者和等级，空格分隔的条件同时匹配；`Ctrl+F` 聚焦。点击「无音乐练习」或「自动」，也可从编辑器根目录直接定位：

```sh
./build/editor --project application/OpenSekai_Community_ASMR --app-chart official-0001_01-easy --app-screen play
./build/editor --project application/OpenSekai_Community_ASMR --app-chart official-0001_01-expert --app-screen auto --app-only
```

键位按实际键盘位置分成三排，每排 12 列。中排普通键为 `A S D F G H J K L ; ' Enter`；上排划动键为 `Q W E R T Y U I O P [ ]`；下排划动键为 `LShift Z X C V B N M , . / RShift`。不使用 Ctrl。按下中排键点击，按住长条，松手判定普通长条尾。键盘划动要求先按下、松开中排键，再按同列任一划动键；普通动作须在 180 毫秒内完成，长条尾端允许此前持续按住，但松开至划动键仍须在 180 毫秒内。上下排同列等效，单独按它们不能打普通音符或承接长条。按键重复、错误列、超时及暂停前留下的动作不能完成划动。

`FlickDirection` 将键盘已完成手势的 Any 与触摸／摇杆的 Up、Left、Right 分开，保留定向划动的判定差异，供将来的移动端和游戏机输入使用。鼠标目前支持单点点击、按住后的移动和定向划动。`Esc` 暂停，暂停时 `Enter` 恢复、`R` 重试、退格键返回曲库；结果页 `Enter` 返回选曲。失去窗口焦点会自动暂停。鼠标单点不替代多点触摸，键盘支持同时按键。

Tell Your World 的六个难度已补入原版 WAV 与封面，`fillerSec` 为 9 秒。音乐、美术和全部 62 个音效原文件现保存于 Git 追踪的 [resources/opensekai](../resources/opensekai/README.md)。其他没有音乐的谱面继续标明「无音乐练习」，练习跳过音乐前置空白。音频栏选择 OGG/MP3/WAV 后使用 FFmpeg 解码为 48 kHz 双声道 PCM，再由 SDL 播放；音频消费时钟减去 manifest 的 `fillerSec`，推进谱面。暂停、重试会同步处理音频。超过 30 分钟或解码失败会明确报错。

判定、空击、划动、摩擦、连接点、长条循环与手动演奏结算已接入原版音效。Live Prefab 的 16 个 cue 加 FINISH 动画事件，共 17 个 cue、18 个片段，在倒计时前解码；音效通过独立 SDL 回调混音，音乐时钟只读取音乐流。长条随按住、松手、暂停和重试同步处理；同拍恰好两个可判定音符按原规则合并相同类别音效，三个同拍音符分别播放。生成的长条连击点不另发声。自动模式默认不播放结算演出音，沿用原 `AutoResultAnimationNone`。音效皮肤切换、角色语音、倒计时/技能等完整演出事件和音量设置入口仍需迁移。

Life 为 0 仍继续接受输入、计分和播放音乐。谱面结束与结果界面分开：最后的音符处理完后等待 1 秒，再播放对应 AP / FC / CLEAR 音效并开始背景淡入，0.2 秒后开始原 prefab 字形动画；FINISH 的声音由原动画第 0.06666667 秒事件触发。按原自制谱面返回管理器路径，演出开始 4 秒后淡出 2 秒，然后显示结果；中间不能以重试键跳过演出。音乐尾段独立于判定状态继续播放。原始 Sprite、层级、位置/缩放/颜色曲线、加权切线、阶跃、光环网格、材质曲线和加法混合已编译执行。13 个粒子配置也已接入：AP／FC／CLEAR 使用各自爆发和循环发射，FINISH 的两个粒子对象保持关闭。粒子的大小曲线、颜色与独立 Alpha 渐变、圆形／盒形发射、固定／自动随机种子来自原资源；速度阻尼使用 60 Hz 视觉模型，原生 Unity 的积分与随机分布仍需用截图校准。

演奏背景现使用原 `Background2DView` 的背景、窗口、封面、反射和地板层。结果页参考用户提供的录屏，先显示通关标题并填充实际评级条，再将封面／曲名栏滑入，随后显示分数、连击和逐行判定统计。数字从零递增到已保存的真实结果，展示动画不修改成绩；音乐在原结算演出结束后停止，结果页使用独立时钟，4.2 秒入场完成后才开放重试／返回。中文与英文使用相同阶段。角色由内置 imagegen 根据录屏重建，使用 [分层 2D 图集与动作数据](../resources/generated/results/rig-v1/README.md)：16 个部件、14 个关节、呼吸／庆祝两个循环，头部、手臂、发梢、领结、蝴蝶结与眼口表情各自运动。结算完成后继续播放，Life 为零使用较轻的待机动作；结果数据保持不变。它是可复用的 2D 骨骼机制，立体姿态和衣服形变仍与原模型有差异。当前矩形裁剪尚未复现圆角 SpriteMask，背景透视纹理插值及 HUD 字体／渐变也需继续核对。

`make opensekai-result-check` 在隔离的硬件 Vulkan 窗口执行四种手动结算，保存三个动画时刻和结果截图，并将实际混音 PCM 与各自原 WAV 作相关性比对。需要 NumPy、FFmpeg 和 labwc；使用临时谱面与 SDL dummy 输出。报告保存在 `build/opensekai-results/report.json`。`--app-captures DIR` 可把 F12 截图按帧号保存，结束截图仍使用 `--output`。

用户的《里表情人》封面、MP3、录屏和 17 张原分辨率切片保存在 [resources/references/0114_01](../resources/references/0114_01/README.md)。五个难度已接入封面及 [112.144 秒游戏用 WAV](../resources/audio/0114_01/README.md)。音频去掉录屏中跳过的中段，`fillerSec` 校准为 1.763 秒；对齐仅使用正式演奏片段，选曲页预览不参与起点判断。原 MP3 保留。视觉验收比较相同可见状态的颜色、尺寸、字形、透视和特效层次，允许 Rust 使用自己的数据结构与逻辑。

选曲页的「音符流速 / Note speed」支持 1.0–12.0，按 0.1 调整，自动保存在本地 `native-play-settings.json`。流速改变音符接近判定线的可视时间，不改变音乐、判定窗口或谱面数据。音符与长条共用连续的远端投影，仅剔除不足亚像素大小的几何；尾端进入视野时保持贴合，避免将地平线外的连线夹到轨道中部。九切片随轨道平面投影，普通音符命中时立即移除。长条采用实际演奏的 `longNoteLine.png`，三个纹理带共享边缘，按原 NoteLineView 的宽度和时间缓入／缓出采样；连续区段也裁切到判定线，按住时保留持续按压标记，尾端结束后移除连线。ASM 材质通过图集两处 RGB 混合保留原版颜色脉动和基础 alpha，未改动谱面编辑器的预览网格。

分数、生命、连击和判定使用 FrontUIView 的原版图集字形与布局。加分提示只由成功命中的可见音符触发，显示本次实际增分，沿原 0.2 秒 OutQuad 动画进入并在 0.5 秒内淡出；长条内部判定点继续计分，不反复固定这个提示。中排按键和鼠标接触引发白色列闪光，0.18 秒内消退，持续按住不保持常亮。上滑标记使用自己的原始尺寸，悬浮在普通块上方，每秒两个周期上升、淡入淡出和复位，不再随底块拉伸。

打击光柱、键面图案、扩散光环和划动爆闪分别执行原粒子层，持续长条使用自己的循环发射，松手即停。编译器保留源相机、层级变换、粒子枢轴、图集选择、颜色／大小曲线、发射和各层前后顺序；材质根据 Custom1.x 分别使用透明或加法混合，避免背景光片盖暗纹案或全部叠加成白块。锥体／球体粒子运动、随机分布和辉光仍使用 Rust／ASM 的视觉实现，需要继续按录屏校准。

从编辑器根目录生成实际窗口与录屏的并排图：

```sh
make opensekai-result-check OPENSEKAI_RESULT_ARGS='--prefix opensekai-reference-endings --cover application/OpenSekai_Community_ASMR/content/library/0114_01-master/jacket.png'
make opensekai-result-check OPENSEKAI_RESULT_ARGS='--prefix opensekai-result-motion --motion --cover application/OpenSekai_Community_ASMR/content/library/0114_01-master/jacket.png'
make opensekai-result-check OPENSEKAI_RESULT_ARGS='--prefix opensekai-result-rig --motion --character --cover application/OpenSekai_Community_ASMR/content/library/0114_01-master/jacket.png'
python3 tools/compare_opensekai_reference.py
make opensekai-gameplay-check OPENSEKAI_GAMEPLAY_ARGS='--prefix opensekai-gameplay-reference'
make opensekai-gameplay-check OPENSEKAI_GAMEPLAY_ARGS='--prefix opensekai-gameplay-feedback --feedback-details'
make opensekai-scoring-check OPENSEKAI_SCORING_ARGS='--prefix opensekai-dynamic-score'
```

结算并排图输出到 `build/opensekai-reference-endings/comparison.png` 和 `comparison.json`。演奏对照输出到指定 prefix 目录的 `comparison.png` 和 `report.json`：手动按住首个长条，再运行真实 Auto 演奏，抓取粉色划动、蓝色点击和弯曲长条。截图按实际音乐样本时钟触发，报告保留请求和实际播放位置；Auto 的连击与判定文字不同于录屏的手动演奏。对照流速 11.3 用于匹配首个长条深度，录屏的精确设置未知，应用默认仍为 6.0。

`--feedback-details` 另以默认流速 6.0 输出 `feedback.png`：真实空击后保持按住，检查列闪光消退；持续按住官谱的首个金色长条，检查无新头块时的光柱／纹案及松手停止；截取首个划动标记的多个上浮周期。它提供实际窗口的视觉检查材料，自动检查不宣称图片逐像素一致。

并排图裁掉宿主窗口的留黑，只缩放应用画布；没有将参考截图作为运行时背景。`make opensekai-visual-check` 使用真实鼠标点击设置流速并截取演奏状态。图像对照仍有差异，HUD 辉光、打击粒子分布和光束外形仍需继续校准。

`python3 tools/capture_results.py` 重新执行原 C# 结果选择的 96 组组合，并从项目自带 DOTween DLL 验证淡出缓动枚举；Rust 回归与该独立结果比较。`native/baseline/results.json` 记录源码哈希，只有显式 `--capture` 才重建。

所有谱面使用 `ojsk-dynamic-v1` 统一计分规则，手动、自动和无音乐练习共用一个实现：

| 判定点 | 基础权重 |
| --- | --- |
| 普通点击、长条首尾 | 1 |
| 划动、划动尾 | 1.25 |
| 连接点、摩擦、摩擦长条 | 0.5 |
| 长条内部自动连击点 | 0.1 |
| 金色音符 | 上述权重乘 2 |

单次贡献为「权重 × 判定质量 × 连击加成」：JP／Auto 为 100%，PERFECT 为 90%，GREAT 为 70%，GOOD 为 50%，BAD／MISS 为 0。连击加成为 `1 + C / (2 × N)`，`N` 是本谱判定点总数，`C` 是该拍开始前的当前连击数；同拍音符共用加成，断连后从基础加成重新积累。分母按该谱全部理想命中的贡献预先归一，正常谱面顺序下全 JP／Auto 的标准满分恰为 1,000,000。内部用整数分数累计，再对累计成绩取整，避免每次命中丢失小数；相邻音符因权重相同或取整仍可能得到相同增分，不人为随机。隐藏长条点计分但不单独触发加分提示，界面提示仅显示本次可见命中的实际增分。

演奏结束写入该包的 `native-last-result.json`，包含 `scoringRule`、自动／无音乐练习标记及实际成绩；旧成绩不会擅自重算，这还不是原版最佳成绩历史存储。全库 `audit-library` 同时检查计数和每张谱面的 Auto 百万分归一，是解析与自动判定逻辑检查，不等于 3,507 次实机窗口游玩。`--motion` 额外保存结果页评级、封面入场、数字递增、最终值的实际窗口切片，输出 `result-motion.png`。`--character` 保持结算窗口 22 秒，采集成绩完成后的不同角色姿态，输出 `result-character.png`。

`opensekai-live-check` 使用临时小谱，经真实 SDL 键盘事件完成点击、长条、松手、上排划动、左 Shift／右 Shift 划动、暂停与结算，并检查单独划动键不能抢走普通音符；以明确标识的测试音验证音乐时钟，同时采集原版音效混音 PCM、循环启停与丢弃计数。`opensekai-song-check` 使用临时副本，完整运行 Tell Your World MASTER 的音乐、音效与自动判定；默认在独立的 labwc Wayland 会话中打开原生 Vulkan 窗口，避免桌面焦点切换触发暂停，需要安装 labwc。`OPENSEKAI_SONG_ARGS=--desktop` 可在当前桌面运行。宿主 `--app-exit-on-result` 在完成结算后截图退出，另设时限防止测试无限等待。两者使用 SDL dummy 输出，不声称验证物理扬声器。

`ASMUI_CAPTURE_SFX=/path/output.f32` 可保存最近一次演奏前 30 秒的 48 kHz 双声道 float32 音效输出，离开或关闭时写盘，不在音频回调内写文件。`ASMUI_PROFILE_AUDIO=1` 记录音乐样本时钟、暂停/恢复、焦点事件和音效回调耗时。图形宿主按可用性选择 mailbox，避免 FIFO 显示等待阻塞输入，并将循环限制在 120 Hz；判定不依赖帧数。字形和音符贴图在倒计时前准备，中文与数字分别按各自文本上下文生成字形。`ASMUI_PROFILE_FRAMES=1` 输出更新、文字布局和 GPU/显示分段耗时。

`make sound-check` 检查宿主混音、PCM 所有权和 SDL 回调启停。可使用 ASan/UBSan 构建重复运行；实际 Vulkan 窗口检查使用 `ASAN_OPTIONS=detect_leaks=0`，不将图形驱动退出时的分配作为应用内存证据。独立音效检查保留 LeakSanitizer，Rust 共享库不因 C 宿主启用 ASan 而自动得到插桩。

窗口回归使用真实 SDL/Vulkan 窗口，检查中文输入落盘、设置弹窗关闭、1280×800 帧缓冲、进入谱面编辑器、放置音符、Ctrl+Z/Y/S 与保存字段；报告与截图写入编辑器的 `build/opensekai-native-smoke.*`。可通过 `OPENSEKAI_WINDOW_ARGS='--validation-root /path/to/usr'` 加载指定 Vulkan 验证层。它只验证原生宿主行为，不代替原 Unity 的画面对照。

谱面编辑器直接执行原 prefab 的 RectTransform、网格/线性布局、裁剪、Sprite 图集和九宫格数据。可放置独立音符、长条和引导线首尾、选择、拖动、删除、撤销/重做、保存；主轨道按原版分段绘制长条和缓入/缓出曲线。删除任一长条端点会删除整条，删除内部连接点则重连相邻点。右侧小地图可定位，时间范围按原版 BPM 与曲目时长计算。`--app-screen maker` 打开当前选中谱面；没有有效谱面时报告原因。重复宿主导航保留当前编辑会话。

```sh
cargo run --manifest-path application/OpenSekai_Community_ASMR/Cargo.toml -- inspect /path/to/score.json
python3 application/OpenSekai_Community_ASMR/tools/capture_baseline.py
python3 application/OpenSekai_Community_ASMR/tools/import_unity_assets.py
python3 application/OpenSekai_Community_ASMR/tools/capture_minimap.py
python3 application/OpenSekai_Community_ASMR/tools/capture_long_notes.py
```

重新采集基准必须显式传 `--capture`，且需要 .NET 10 SDK 执行原 C#。`capture_baseline.py --capture-oracle` 只更新行为数据，先检查原文件哈希；`capture_minimap.py` 默认重新执行原 C# 并比较，只有 `--capture` 才写入结果。普通文件检查不覆盖基准。资源导入器需要 PyYAML；输出保留 GUID、fileID、层级、组件字段和素材路径，报告缺失外部引用。导入了数据不意味着已实现对应组件。

## 迁移验收范围

| 范围 | 当前证据 / 尚需工作 |
| --- | --- |
| 原始资源与格式 | 哈希固定；Manifest、谱面字段、Newtonsoft 引用、BPM、SUS 与 3,507 张真实官谱已接入 Rust；原 C# SUS 合并和表格计数有独立对照 |
| 编辑器接入 | 版本化 C ABI、动态库加载、Game 视图、输入与独立字体已接通；项目资源树、检查器、播放控制仍需绑定应用实际状态 |
| 谱面管理 | 布局、扫描、搜索、选中、新建、中文输入、保存、滚动、复制、ZIP 导入导出、原生文件选择与 SUS/JSON 替换已接通；删除确认、封面显示/740px 转换、时长和完整设置仍需迁移 |
| 谱面编辑器 | 原 prefab 布局、图集、独立音符及长条首尾放置、曲线连线、撤销/重做、量化、保存和缩略谱面已接通；连接点插入、完整选择/变形操作、事件编辑、波形、音频与试游玩仍需迁移 |
| 演奏 | 点击、长条/连接点/尾部、摩擦/划动、自动、判定窗口、计分/生命、音乐时钟、判定/循环/结算音效、暂停/重试/返回和结果落盘已接通；全库计数匹配。仍需完整对齐每种输入状态、原版长条/Guide 网格材质、完整演出与音效事件、特效、设置与最佳成绩历史 |
| 视觉资源和特效 | 索引保留 1,553 个资产、16,070 个对象，报告 111 个外部 GUID；谱面 bundle 包含 6 个 prefab、573 个 Sprite 和 139 张纹理，另编译 5 个结算 prefab 的默认动画。当前界面执行已用到的 Sprite/九宫格、字形、裁剪、演奏打击层与结算字形/光环；通用平铺、完整 TMP 排版、动态状态、其他材质、通用动画、其他粒子、MV 和完整过场仍需迁移 |
| 视频和数据工具 | 仍需迁移视频生成、备份恢复、格式转换等原版入口 |
| 视觉不变 | 目前只有源码参数与 Rust/ASM 渲染证据；仍需原 Unity 执行参照，或逐组件独立参考与差异解释，不能以自产图自证 |

所有未接通入口应明确报出迁移状态，不伪报成功。源码许可与第三方字体、音频、游戏资源许可分别保留，资产不改写为 Rust 代码的许可。
