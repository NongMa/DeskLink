$msi = 'F:\\AI Project\\VoiceRemoteControl\\packaging\\out\\DeskLink-Setup-0.1.0.msi'
$target = 'C:\\tmp\\desklink-test'
msiexec /a $msi /qn TARGETDIR=$target
Start-Sleep 5
Write-Host "--- extracted ---"
if (Test-Path $target) {
    Get-ChildItem $target -Recurse | Select-Object FullName, Length | Format-Table -AutoSize | Out-String -Width 300
} else {
    Write-Host "target dir not created"
}