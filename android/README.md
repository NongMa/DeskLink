# 桌链 DeskLink · 手机端（Android）

小米电视式语音遥控的 Android 客户端：与 PC 端（`pc/`）局域网配对后，
提供语音指令、scrcpy 投屏镜像、侧面板快捷键与手机文件管理。

## 功能

- 语音控制：端侧离线识别（sherpa-onnx + SenseVoice int8），指令经 WiFi 发往 PC 端
- 投屏镜像：PC 端 scrcpy 5.0.1 回传手机画面，App 内侧面板操控
- 侧面板：快捷控制（媒体键 / 音量 / 常用应用）、鼠标控制（触摸板 / 左键 / 右键）
- 文件管理：手机与 PC 双向互传

## 技术栈

- Kotlin 2.2.x + Jetpack Compose（BOM 2026.02.01）
- minSdk 31 / targetSdk 37 / compileSdk 37，仅 arm64-v8a
- sherpa-onnx 2.0.1，本地 AAR：`app/libs/sherpa-onnx-2.0.1.aar`（随仓库分发，不发布到公共仓库）
- SenseVoice int8 模型随 APK 分发（`noCompress "onnx"`，跳过无收益的再压缩）
- 依赖锁定：`kotlin-stdlib` 强制 2.3.21（AGP 9 内置编译器读不了 metadata 2.4.0）

## 目录

```
android/
├── app/                  模块源码（src/main/java/cd/cc/nongma/vrc/…）
│   ├── libs/             sherpa-onnx 本地 AAR
│   └── build.gradle.kts  ABI 过滤 / noCompress / 签名（debug）
├── build.gradle.kts      根构建脚本
├── settings.gradle.kts
└── gradle/wrapper/…
```

## 构建

```sh
./gradlew assembleRelease
# 产物：app/build/outputs/apk/release/app-release.apk
# 注意：release 变体暂用 debug 签名（项目暂无独立 release 密钥），仅本地自用
```

## 使用

1. PC 端装好 DeskLink（见 `pc/README.md`），手机与 PC 同一 WiFi。
2. 打开应用 → 连接面板输入 PC 地址（如 `192.168.x.x:8765`），或扫描 PC 端 `/admin` 页二维码。
3. 授予麦克风权限；按住说话，松开发送。

## 大文件（不入 git，需自行准备）

- `app/src/main/assets/sense-voice/model.int8.onnx`（约 226 MB）：从 k2-fsa/sherpa-onnx
  的 SenseVoice 发行包（`sherpa-onnx-sense-voice-zh-en-ja-ko-yue-*`）获取，放入同名路径；
  仓库已带 `tokens.txt`，只缺 onnx 本体
- `app/libs/sherpa-onnx-2.0.1.aar`（49 MB）：**已随仓库分发**，无需另取

## 已知问题与取舍

- release 构建**禁用了 R8 的库范围优化**（AGP 9 `optimization` DSL 曾对 `kotlin.**`
  过度内联，运行期 `IllegalAccessError: kotlin.sequences.d …` 闪退，已在
  `app/build.gradle.kts` 移除）。
- 端侧识别模型较大，APK 约 273 MB。
- 应用数据目录 `%APPDATA%\VoiceRemoteControl`（PC 端）保持原名不动；见根 `README.md` 命名红线。
