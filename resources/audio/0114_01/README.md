# 里表情人 / 裏表ラバーズ — 游戏用音频

`music-game.wav` 从用户提供的完整版 MP3 解码为 48 kHz 双声道 float32，保留开头至约 96.855 秒和约 169.685 秒之后的样本，中间去掉约 72.830 秒。剪接位置通过用户录屏的波形确认；音乐没有混入录屏中的打击音，也没有再次进行有损压缩。

五个 `0114_01` 难度共用这个音乐版本。谱面第一个音符为 tick 0，`fillerSec` 校准为约 1.763 秒。原元数据中的 9 秒针对另一份音频，保存在 manifest 的 `sourceMetadataFillerSec` 中。原 MP3、录屏和封面仍保存在 [参考资料](../../references/0114_01/README.md)。

```sh
python3 tools/prepare_0114_audio.py prepare
python3 tools/prepare_0114_audio.py verify
```

需要 FFmpeg、NumPy 和 SciPy。脚本核对原文件 SHA-256，验证保留段样本逐个相同，检查剪接前后九段音乐与录屏的波形相关性，并核对五个谱面包；不会覆盖已有的不同音频。裁剪参数见 [edit.json](edit.json)，验证结果见 [verification.json](verification.json)。视频定位存在约 40 ms 的不确定性，可在编辑器中继续调节音频偏移。

The game edit removes the recording's skipped middle section from the preserved user MP3. Its calibrated lead-in aligns chart tick zero. The float WAV retains the decoded samples exactly; the recording's tap sounds are excluded. All five difficulties receive the same edit, while the original media remains intact.
