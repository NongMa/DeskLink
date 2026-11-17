# 桌链 DeskLink

手机端（Android）与 PC 端（Windows）协同。

- 手机端文档：[android/README.md](android/README.md)
- PC 端文档：[pc/README.md](pc/README.md)
- `appicon/`：两端共用的双环相链图标生成脚本

## 构建产物速查

- 手机端：`android/app/build/outputs/apk/release/app-release.apk`（debug 签名，本地自用）
- PC 端：`pc/packaging/out/DeskLink-Setup-0.1.0.msi`

## 第三方许可

- scrcpy（Apache 2.0，原样分发）、Google platform-tools（adb）、whisper.cpp（MIT）
- sherpa-onnx、SenseVoice（端侧语音识别，随 APK 分发）
- WiX Toolset v7（仅打包期间使用；年收入 >$10k 的组织须赞助 wixtoolset，个人/自用无碍）
