# 分层 2D 结算角色

`atlas-v2.png` 是内置 `image_gen.imagegen` 生成的透明 RGBA 部件图，以用户提供的 [原应用结算截图](../../../references/0114_01/frames/result.png) 和上一版角色插画为参考。它是获得用户授权的重建素材，不是原应用模型或贴图。原始输出保留不变；`atlas-v1.png` 保留首次生成版本，两次完整提示词分别见 [prompt-v1.md](prompt-v1.md) 和 [prompt-v2.md](prompt-v2.md)。

实际图集大小为 1254×1254，部件没有严格均匀排列。工具生成的透明边缘仍有极低 Alpha 的 RGB 残留，运行时保留原 Alpha；坐标测量忽略低 Alpha 背景，未裁剪、重绘或修改 PNG。`rig.json` 记录准确的 16 个部件区域、绘制顺序、14 个关节和两个循环动作，以及当前图集 SHA-256。

躯干、头部、上臂、带袖前臂、发梢、领结、蝴蝶结、眼睛和嘴分别绑定关节。父子变换共用肩肘连接点，呼吸、歪头、双手庆祝和饰物摆动由独立曲线控制；眼睛和嘴使用离散表情槽，眨眼时不替换身体。当前动作是 `idle`（轻微呼吸和眨眼）与 `celebrate`（庆祝、笑脸和摆动），各自 6.4 秒循环。结果页 Life 为零时使用 `idle`，其它情况使用 `celebrate`；结算完成后仍持续运动。

从项目根目录重建坐标和动作数据：

```sh
python3 tools/compile_result_rig.py
cargo test --locked --test rig2d --test result_ui
```

工具只读取图集来测量 Alpha 边界，然后写 JSON。若更换图集设计或尺寸，需重新设定区域、局部尺寸和连接点，不能沿用旧网格坐标。Rust 的 [rig2d.rs](../../../../native/src/rig2d.rs) 不依赖这个角色；另一角色可使用同一 schema 的图集、骨骼、图层和曲线。

真实窗口验证在 ASM 编辑器根目录运行：

```sh
make opensekai-result-check OPENSEKAI_RESULT_ARGS='--prefix opensekai-result-rig --kind full_combo --motion --character --cover application/OpenSekai-ASMR/content/library/0114_01-master/jacket.png'
```

`--character` 在成绩显示完成后继续运行，采集睁眼、闭眼、开口与摆动姿态；`result-character.png` 是实际 Vulkan 窗口的切片。独立测试验证关节连接、动作循环接缝、表情变化和最终成绩不变。生成部件的立体感、侧脸和服装形变仍与原 3D 模型有差异，不能由这些检查推出逐像素一致。
