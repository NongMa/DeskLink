$msi = 'F:\\AI Project\\VoiceRemoteControl\\packaging\\out\\DeskLink-Setup-0.1.0.msi'
& wix msi decompile $msi 2>&1 | Select-Object -Last 20
Get-ChildItem 'F:\\AI Project\\VoiceRemoteControl\\packaging' -Filter '*.wxs' | Select-Object Name, Length