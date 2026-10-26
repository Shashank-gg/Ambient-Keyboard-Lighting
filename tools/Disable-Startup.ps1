$startup = [Environment]::GetFolderPath('Startup')
$shortcutPath = Join-Path $startup 'Ambient Keyboard Lighting.lnk'
if (Test-Path $shortcutPath) {
    Remove-Item $shortcutPath -Force
    Write-Host 'Ambient keyboard lighting startup has been disabled.'
} else {
    Write-Host 'Startup shortcut was not found; nothing to remove.'
}
