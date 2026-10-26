# Ambient Lighting Edition

This fork is narrowed to the AmbientLight effect. The GUI exposes the ambient effect's FPS target (1–100) and saturation boost; other effect choices and saved custom-effect UI are removed from the main screen. Existing settings are migrated to AmbientLight on launch.

## Windows build

GitHub Actions builds a Windows release when code is pushed to `main`, or when you manually run **Build Ambient Lighting for Windows** from the Actions tab. Download the `Ambient-Keyboard-Lighting-Windows` artifact from a successful run and extract it.

## Automatic startup

Keep `legion-kb-rgb.exe` and the two PowerShell scripts in the same folder. Run `Enable-Startup.ps1` to add a shortcut to the current Windows user's Startup folder; run `Disable-Startup.ps1` to remove it. Startup launches the program with `--gui --hide-window`, intended to keep it in the tray. If your tray/window environment does not support hiding, the app may remain visible.

## Performance and compatibility

The ambient capture loop is configured for up to 100 FPS, and the slider is capped at 100. This is a requested processing rate; screen capture scheduling, Windows composition, USB polling, firmware, and keyboard controller response can limit actual LED update frequency. This package has not been tested on the user's physical LOQ 15IRX9, so verify keyboard support and performance on-device.

## License

This is a modified derivative of L5P-Keyboard-RGB / Legion RGB Control. The upstream repository includes a GPL-3.0 license; retain the license and comply with its redistribution/source-code requirements when publishing binaries or modified source.
