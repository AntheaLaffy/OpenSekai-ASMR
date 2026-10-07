# 里表情人视觉基准

用户提供的封面、完整版 MP3 和原应用录屏保存在此处，原文件按 SHA-256 核对。录屏为 1280×720、60 FPS；画面显示 MASTER、1151 Combo、FULL COMBO。完整音频和录屏演奏段长度不同。

演奏用音频已按正式演奏段裁剪，见 [游戏用 WAV 与剪接参数](../../audio/0114_01/README.md)。选曲页的音频预览不参与起点校准；依据正式演奏的首枚音符和中段、末段音乐定位。

`frames/` 保存原分辨率切片，覆盖选曲、普通打击、粉色划动、金色 Critical、长条、同时打击，以及结算光环、爆发粒子、稳定字形、淡出和结果页。每张图的来源位置见 [reference.json](reference.json)。

**验收重点是视觉外观。** 使用相同可见状态比较构图、尺寸、颜色、透明度、字形、轨道透视和特效层次。Rust 的谱面结构、输入和判定逻辑可以沿用；录屏时间标记仅用于找回画面，不要求复制其时钟或程序结构。

```sh
python3 tools/import_visual_reference.py verify
python3 tools/import_visual_reference.py attach-cover
```

素材由用户提供，独立于 `resources/opensekai/upstream` 的原仓库快照。根目录的原始文件保留。

Use equivalent visible states for visual comparison. These images are reference frames, not replacement gameplay backgrounds or a timing contract. Missing art can be generated with the imagegen skill after checking the original assets.
