# Run from PowerShell in the folder containing legion-kb-rgb.exe.
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'legion-kb-rgb.exe'
if (-not (Test-Path $exe)) {
    throw "legion-kb-rgb.exe was not found next to this script. Download the Windows build into this folder first."
}
$startup = [Environment]::GetFolderPath('Startup')
$shortcutPath = Join-Path $startup 'Ambient Keyboard Lighting.lnk'
$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut($shortcutPath)
$shortcut.TargetPath = $exe
$shortcut.Arguments = '--gui --hide-window'
$shortcut.WorkingDirectory = $PSScriptRoot
$shortcut.Description = 'Start ambient keyboard lighting when Windows starts'
$shortcut.Save()
Write-Host "Startup enabled. Shortcut created at: $shortcutPath"
Write-Host 'The app will start in the background/tray at your next sign-in.'
