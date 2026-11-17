# 桌链 DeskLink · PC 端（Windows）

手机端（`../android/`）的 Windows 协同一端：局域网配对、语音指令执行、
scrcpy 投屏回传、手机文件管理。

## 组件

| 目录 | 组件 | 说明 |
|---|---|---|
| `windows-server/` | `vrc-server.exe`（Rust） | 常驻托盘 agent。HTTP/WS 服务端口 **8765**；`/admin` 仅监听 127.0.0.1 |
| `tauri-admin/` | `vrc-admin.exe`（Tauri v2） | 管理面板壳：窗口加载 `http://127.0.0.1:8765/admin`（QR / 配对 / 设置） |
| `packaging/` | WiX v7 MSI | `build-msi.ps1` 生成 `DeskLink-Setup-0.1.0.msi` |

## windows-server 模块（src/）

- `server.rs` HTTP + WebSocket API；`admin.rs` /admin 页 + QR + token
- `input.rs` 键盘 / 鼠标 / 媒体键注入；`panel.rs` 侧面板指令
- `mirror.rs` scrcpy 投屏编排（`vendor/scrcpy`，未修改源码）
- `fileman.rs` 手机文件管理（adb）；`appcat.rs` 应用分类
- `audio.rs` + whisper-rs：手机端未识别时的 PC Whisper 兜底（模型在 `models/`）
- `tray.rs` 托盘（`assets/vrc.ico` 双环相链，编译期嵌入）；`autostart.rs` HKCU Run 自启（默认关）
- `state.rs` / `config.rs` / `logbuf.rs` / `main.rs`

## 安装布局（MSI per-machine）

装到 `%LOCALAPPDATA%\Programs\DeskLink\`：

```
vrc-server.exe        嵌入 PE 图标（build.rs + embed-resource 编译期嵌入 assets/vrc.ico）
vrc-admin.exe
models\ggml-small-q5_1.bin
vendor\scrcpy\…              scrcpy 5.0.1 原样分发
vendor\platform-tools\…      adb 1.0.41
```

- 开始菜单「桌链 DeskLink」快捷方式（公共开始菜单，图标 = icon_vrc）
- 桌面快捷方式（公共桌面，同图标；per-machine 安装后自动创建）
- 控制面板卸载条目带图标（ARPPRODUCTICON）
- 安装需 UAC 提权（per-machine，与 Tauri 同款）；HKCU 自启 MSI 不配置

## 构建

```sh
# 0. PATH 前置 cargo（避免 ServBay/rustup 混排）
$env:PATH = "C:\Users\$env:USERNAME\.cargo\bin;" + $env:PATH

# 1. vrc-server（whisper-rs-sys 需 cmake；pc/.cargo/config.toml 已指向 VS BuildTools 自带 cmake）
cargo build --release --manifest-file pc\windows-server\Cargo.toml

# 2. vrc-admin（Tauri v2，bundle 关闭，只产出 exe）
cargo build --release --manifest-file pc\tauri-admin\src-tauri\Cargo.toml

# 3. MSI（WiX v7）
wix eula accept wix7
wix extension add WixToolset.UI.wixext
powershell -NoProfile -ExecutionPolicy Bypass -File pc\packaging\build-msi.ps1
# 产物：pc/packaging/out/DeskLink-Setup-0.1.0.msi
```

## 大文件（不入 git，获取方式）

- `pc/windows-server/models/*.bin`：whisper.cpp 量化模型，从 huggingface `ggerganov/whisper.cpp`
  下载（如 `ggml-small-q5_1.bin`），放入 `windows-server/models/`
- `pc/windows-server/vendor/`：scrcpy 5.0.1 + Google platform-tools，见
  `pc/windows-server/vendor/README.md`

## 命名红线

见根 `README.md`：可见文案只用「桌链 / DeskLink」；applicationId / exe 名 /
`vrc://` / 注册表 Run 项 / `%APPDATA%\VoiceRemoteControl` 一律不动。

## 已知取舍

- MSI 未签名（个人自用）；WiX v7 个人年收入 < $10k 免费
- `wix msi validate` 仅 ICE91 22 条（per-machine 装 LocalAppData 的固有警告，可接受）；
  ICE38/ICE64 已用 HKCU KeyPath + RemoveFolder 规避
- 产品码每构建变化，升级靠固定 UpgradeCode
- `pc/.cargo/config.toml` 与 `pc/windows-server/.cargo/config.toml` 的 CMAKE 路径是本机路径
