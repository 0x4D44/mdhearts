# WinUI 3 Host Checklist

_Last updated: 25 September 2025_

## Prerequisites
- Visual Studio 2022 17.10+ with:
  - **Desktop development with C++** workload
  - **Universal Windows Platform development** workload
- Windows App SDK 1.5+ runtime and MSIX packaging tools (`winget install Microsoft.WindowsAppSDK.1.5`)
- Windows App SDK NuGet packages cached locally (`nuget.exe install Microsoft.WindowsAppSDK -Version 1.5.240904000`)
- `windows` crate feature set enabled via `cargo build --features winui-host -p hearts-app`

## Integration Steps
- Enable the host with `cargo run -p hearts-app --features winui-host --bin mdhearts` to verify the placeholder view (shows a Win32 dialog while we flesh out XAML).`n
1. **Bootstrap WinUI app:** Replace `platform::run()` routing with `platform::winui::run()` and enable the `winui-host` feature when building.
2. **XAML assets:** Add `app.xaml` and `MainWindow.xaml` under `crates/hearts-app/winui/`. Include them via `include_str!` or compile into a Windows App SDK project and load with `Application::LoadComponent`.
3. **Window factory:** In `platform/winui.rs`, create `App` and `MainWindow` structs using `implement::App` macros from `windows` crate.
4. **Resource deployment:** Ensure `assets` directory is copied alongside the MSIX or added to the WinUI project as loose files.
5. **MSIX packaging:** Use `makeappx.exe` or Visual Studio Packaging Project referencing the `mdhearts.exe` binary.

## Testing
- Run `cargo run -p hearts-app --features winui-host` to validate the WinUI host can create a window.
- Use `WinAppDriver` smoke tests to verify startup and window focus.

## References
- [Windows App SDK documentation](https://learn.microsoft.com/windows/apps/windows-app-sdk/)
- [windows-rs WinUI 3 sample](https://github.com/microsoft/windows-app-rs/tree/main/samples/winui)
