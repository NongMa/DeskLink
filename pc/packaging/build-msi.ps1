# 桌链 DeskLink MSI 打包脚本（WiX v7）
# 布局：%LOCALAPPDATA%\Programs\DeskLink\
#   vrc-server.exe / vrc-admin.exe / models\ggml-small-q5_1.bin
#   vendor\scrcpy\... / vendor\platform-tools\...
# 开始菜单快捷方式 -> vrc-server.exe（托盘 agent）
# per-machine 安装（WiX 自动置 ALLUSERS=1，双击弹 UAC 后显示分步向导）
$ErrorActionPreference = "Stop"

$root      = Split-Path $PSScriptRoot -Parent
$serverDir = Join-Path $root "windows-server"
$tauriExe  = Join-Path $root "tauri-admin\src-tauri\target\release\vrc-admin.exe"
$serverExe = Join-Path $serverDir "target\release\vrc-server.exe"
$vendorDir = Join-Path $serverDir "vendor"
$modelFile = Join-Path $serverDir "models\ggml-small-q5_1.bin"
$outDir    = Join-Path $root "packaging\out"
$wxsPath   = Join-Path $root "packaging\DeskLink.wxs"
$msiPath   = Join-Path $outDir "DeskLink-Setup-0.1.0.msi"

foreach ($f in @($tauriExe, $serverExe, $modelFile)) {
    if (-not (Test-Path $f)) { Write-Error "缺少文件: $f" }
}
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

# 固定 GUID（全 hex），避免 WiX v4 auto-gen 报错
$guidRoot = "{C428A110-8B3A-4A5B-9D7E-F1A2B3C4D5E6}"
$guidScr  = "{B2345678-90EF-1234-ABCD-FEDCBA987654}"
$guidPlat = "{C3456789-0DEF-1234-5678-9ABCDEF01234}"
$guidMenu = "{D4567890-1234-5678-ABCD-EF0123456789}"
$guidDesktop = "{F7890123-4567-89AB-CDEF-0123456789BC}"
$guidVen  = "{E5678901-2345-6789-BCDE-F0123456789A}"

function FileLines($paths, $indent) {
    $pad = " " * $indent
    $out = @()
    foreach ($p in $paths) {
        $id = "fil_" + [guid]::NewGuid().ToString("N").Substring(0, 16).ToUpper()
        $out += "$pad<File Id=`"$id`" Source=`"$p`" />"
    }
    return ($out -join "`r`n")
}

# 每个用户配置文件组件配一个 HKCU 注册表 KeyPath（ICE38 要求）
function RegKey($name, $indent) {
    $pad = " " * $indent
    return "$pad<RegistryValue Root=`"HKCU`" Key=`"Software\DeskLink\Install`" Name=`"$name`" Type=`"integer`" Value=`"1`" KeyPath=`"yes`" />"
}

# vendor 目录块（children 内联），最后一个子元素前再补一个 RemoveFolder 放这里会更麻烦，
# 所以 vendor 子目录的 RemoveFile 由父组件 dir_vendor 统一处理
function VendorFiles($dir, $indent) {
    $files = (Get-ChildItem $dir -File | Sort-Object Name).FullName
    if ($files.Count -eq 0) { Write-Error "vendor 目录为空: $dir" }
    return (FileLines $files $indent)
}

$rootXml = FileLines @($serverExe, $tauriExe) 12
$scrcpyXml = VendorFiles (Join-Path $vendorDir "scrcpy") 12
$platXml   = VendorFiles (Join-Path $vendorDir "platform-tools") 12

$wxs = @"
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs" xmlns:ui="http://wixtoolset.org/schemas/v4/wxs/ui">
  <Package Name="桌链 DeskLink" Manufacturer="DeskLink" Version="0.1.0" Codepage="65001"
           UpgradeCode="{3f1a2b7c-9d4e-4f6a-8b2c-1e5d7a9c0f31}"
           InstallerVersion="500">
    <WixVariable Id="WixUILicenseRtf" Value="${root}\packaging\LICENSE.rtf" />
    <Icon Id="icon_vrc" SourceFile="$serverDir\assets\vrc.ico" />
    <Property Id="ARPPRODUCTICON" Value="icon_vrc" />
    <MajorUpgrade DowngradeErrorMessage="已安装更新版本，请先卸载旧版。" />
    <MediaTemplate EmbedCab="yes" />
    <ui:WixUI Id="WixUI_InstallDir" InstallDirectory="INSTALLFOLDER" />
    <StandardDirectory Id="LocalAppDataFolder">
      <Directory Id="LocalAppDataProgramsFolder" Name="Programs">
        <Directory Id="INSTALLFOLDER" Name="DeskLink">
          <Component Id="cmp_Root" Guid="$guidRoot">
$(RegKey "Root" 12)
$rootXml
            <RemoveFolder Id="rm_Programs" Directory="LocalAppDataProgramsFolder" On="uninstall" />
            <RemoveFolder Id="rm_Install" Directory="INSTALLFOLDER" On="uninstall" />
          </Component>
          <Directory Id="dir_vendor" Name="vendor">
            <Directory Id="dir_vendor_scrcpy" Name="scrcpy">
              <Component Id="cmp_dir_vendor_scrcpy" Guid="$guidScr">
$(RegKey "VendorScrcpy" 14)
$scrcpyXml
              </Component>
            </Directory>
            <Directory Id="dir_vendor_platformtools" Name="platform-tools">
              <Component Id="cmp_dir_vendor_platformtools" Guid="$guidPlat">
$(RegKey "VendorPlatformTools" 14)
$platXml
              </Component>
            </Directory>
            <Component Id="cmp_dir_vendor" Guid="$guidVen">
$(RegKey "Vendor" 12)
              <RemoveFolder Id="rm_Vendor" Directory="dir_vendor" On="uninstall" />
              <RemoveFolder Id="rm_VendorScrcpy" Directory="dir_vendor_scrcpy" On="uninstall" />
              <RemoveFolder Id="rm_VendorPlatformTools" Directory="dir_vendor_platformtools" On="uninstall" />
            </Component>
          </Directory>
          <Directory Id="dir_models" Name="models">
            <Component Id="cmp_dir_models" Guid="{F6789012-3456-7890-CDEF-0123456789AB}">
$(RegKey "Models" 14)
              <RemoveFolder Id="rm_Models" Directory="dir_models" On="uninstall" />
              <File Id="fil_model" Source="$modelFile" />
            </Component>
          </Directory>
        </Directory>
      </Directory>
    </StandardDirectory>
    <StandardDirectory Id="ProgramMenuFolder">
      <Directory Id="ProgramMenuDeskLink" Name="桌链 DeskLink">
        <Component Id="cmp_StartMenuShortcut" Guid="$guidMenu">
          <RegistryValue Root="HKCU" Key="Software\DeskLink" Name="StartMenuShortcut" Type="integer" Value="1" KeyPath="yes" />
          <Shortcut Id="scm_DeskLink" Name="桌链 DeskLink" Description="手机与 Windows 协同（语音 / 投屏 / 文件）"
                    Target="[INSTALLFOLDER]vrc-server.exe" WorkingDirectory="INSTALLFOLDER" Icon="icon_vrc" />
          <RemoveFolder Id="rm_StartMenuDeskLink" Directory="ProgramMenuDeskLink" On="uninstall" />
        </Component>
      </Directory>
    </StandardDirectory>
    <StandardDirectory Id="DesktopFolder">
      <Component Id="cmp_DesktopShortcut" Guid="$guidDesktop">
        <RegistryValue Root="HKCU" Key="Software\DeskLink" Name="DesktopShortcut" Type="integer" Value="1" KeyPath="yes" />
        <Shortcut Id="scm_Desktop" Name="桌链 DeskLink" Description="手机与 Windows 协同（语音 / 投屏 / 文件）"
                  Target="[INSTALLFOLDER]vrc-server.exe" WorkingDirectory="INSTALLFOLDER" Icon="icon_vrc" />
      </Component>
    </StandardDirectory>
    <Feature Id="Main" Title="桌链 DeskLink" Level="1">
      <ComponentRef Id="cmp_Root" />
      <ComponentRef Id="cmp_dir_vendor" />
      <ComponentRef Id="cmp_dir_vendor_scrcpy" />
      <ComponentRef Id="cmp_dir_vendor_platformtools" />
      <ComponentRef Id="cmp_dir_models" />
      <ComponentRef Id="cmp_StartMenuShortcut" />
      <ComponentRef Id="cmp_DesktopShortcut" />
    </Feature>
  </Package>
</Wix>
"@

[System.IO.File]::WriteAllText($wxsPath, $wxs, (New-Object System.Text.UTF8Encoding $true))
Write-Host "WXS written: $wxsPath ($($wxs.Length) chars)"

& wix build $wxsPath -ext WixToolset.UI.wixext -acceptEula wix7 -o $msiPath
if ($LASTEXITCODE -ne 0) { Write-Error "wix build failed" }
Get-Item $msiPath | Select-Object FullName, Length
