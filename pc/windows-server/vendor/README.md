# vendor/ — 预编译第三方组件

这些二进制**已入 git**（`clone` 即得，无需下载），随服务整体分发（MSI 打包时一并带走），
仅供桌链 DeskLink 的「投屏」功能使用。

下面的来源 / SHA-256 / 替换步骤是**升级**时用的，不是首次构建的前置条件。

## scrcpy/ — v5.0.1（Genymobile/scrcpy）

- 来源：<https://github.com/Genymobile/scrcpy/releases/download/v5.0.1/scrcpy-win64-v5.0.1.zip>
- 许可证：Apache License 2.0（随包含的 `LICENSE.txt`，Copyright (C) Genymobile）
- 原样运行（未修改源码）：投屏窗口 + `--no-audio --stay-awake`，位置由 `--window-x/y` 固定；
  按键注入走 `platform-tools/adb.exe -s <serial> shell input keyevent <code>`。
- 更新：替换 `scrcpy.exe`、`scrcpy-server`、`SDL3.dll` 与 FFmpeg/libusb/swresample 各 DLL 为同一发行版即可。

## platform-tools/ — Google Platform Tools 37.0.1（adb 1.0.41）

- 来源：<https://dl.google.com/android/repository/platform-tools-latest-windows.zip>
- 许可证：Android Software Development Kit License Agreement 允许随产品再分发（仅限 adb.exe 及其运行时依赖 `AdbWinApi.dll` / `AdbWinUsbApi.dll`）
- 更新：从同一 URL 下载新版 zip，替换这三个文件。

## 升级检查清单

1. 下载新版 zip，确认 SHA-256（GitHub release 页面 / Google 仓库）。
2. 替换上述文件，更新本 README 的版本号。
3. `cargo build` 后手动验证：投屏开始/结束、面板按键（返回/主页/最近/音量/电源/通知栏/截屏）。
