# 桌链 DeskLink

手机端（Android）与 PC 端（Windows）协同。

- 手机端文档：[android/README.md](android/README.md)
- PC 端文档：[pc/README.md](pc/README.md)

## 功能

- 语音遥控：按住说话 → 手机端离线识别（SenseVoice）→ 文本指令经 WiFi 下发 → PC 解析执行
  （打开应用 / 媒体键 / 音量 / 锁屏 / 截图 / 任务视图 / 标签页管理等）
- 边说边识别：录音中每 700ms 分块重解码、实时上屏；PC 输入框聚焦时切换「转文字输入」直接打进输入框
- Siri 式语音浮层：聆听 / 识别中 / 结果 / 未听清（识别失败本地提示，不回退 PC 端识别）
- 投屏：scrcpy 5.0.1 镜像 + adb 按键注入（USB / 同一 WiFi）
- 侧面板：快捷控制（媒体键 / 音量 / 常用应用）、鼠标控制（触摸板 / 左键 / 确定 / 右键）
- 手机文件管理：经 adb 浏览 / 传输手机文件；相册缩略图由 PC 端按需解码
- 管理页：`http://127.0.0.1:8765/admin`（QR 配对 / 令牌认证 / 已连接设备 / 日志 / 设置），Tauri 壳 `vrc-admin.exe` 打开同一页
- 托盘 agent 常驻后台；HKCU Run 自启默认关闭

## 技术实现

| 层 | 实现 |
|---|---|
| 手机端 | Kotlin + Jetpack Compose（液态玻璃 UI，minSdk 31 / target 37）；sherpa-onnx 2.0.1 + SenseVoice int8 端侧识别（`assets/sense-voice/`，`noCompress`）；OkHttp WebSocket；AudioRecord 16 kHz PCM16 |
| PC 端 | Rust + axum / tokio（HTTP + WS，端口 8765；`/admin` 仅 127.0.0.1）；winapi / windows-rs 键鼠与媒体键注入；UI Automation 检测输入焦点；scrcpy 5.0.1 编排 + adb 注入；image 解码缩略图 |
| 通信 | WebSocket JSON 协议（`hello` / `voice_command` / `input_text` / `command` / `focus_check` / `control` / `mouse` …），令牌认证；配对 QR 由 `qrcode` 生成 |
| 打包 | 手机端 Release APK（debug 签名，本地自用）；PC 端 WiX v7 MSI（per-machine + WixUI_InstallDir，公共开始菜单 + 桌面快捷方式，ARPPRODUCTICON） |
| 仓库 | `android/` + `pc/`（windows-server / tauri-admin / packaging）；`vendor/` 二进制入 git，识别模型与 onnx 不入 git |

## 构建产物速查

- 手机端：`android/app/build/outputs/apk/release/app-release.apk`（debug 签名，本地自用）
- PC 端：`pc/packaging/out/DeskLink-Setup-0.1.0.msi`

## 第三方许可

- scrcpy（Apache 2.0，原样分发）、Google platform-tools（adb）
- sherpa-onnx、SenseVoice（端侧语音识别，随 APK 分发）
- WiX Toolset v7（仅打包期间使用；年收入 >$10k 的组织须赞助 wixtoolset，个人/自用无碍）
