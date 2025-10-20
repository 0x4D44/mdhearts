#[cfg(feature = "winui-host")]
use std::cell::RefCell;
#[cfg(feature = "winui-host")]
use windows::Win32::Foundation::HWND;
#[cfg(feature = "winui-host")]
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW};
#[cfg(feature = "winui-host")]
use windows::Windows::ApplicationModel::Core::{
    CoreApplication, IApplicationView, IFrameworkView, IFrameworkViewSource,
};
#[cfg(feature = "winui-host")]
use windows::Windows::UI::Core::CoreWindow;
#[cfg(feature = "winui-host")]
use windows::core::{HSTRING, Result, implement};

#[cfg(feature = "winui-host")]
pub fn run() -> Result<()> {
    CoreApplication::Run(AppSource::new())
}

#[cfg(not(feature = "winui-host"))]
pub fn run() -> windows::core::Result<()> {
    Err(windows::core::Error::new(
        windows::Win32::Foundation::E_NOTIMPL.into(),
        "WinUI host feature disabled",
    ))
}

#[cfg(feature = "winui-host")]
#[implement(
    Windows::ApplicationModel::Core::IFrameworkViewSource,
    Windows::ApplicationModel::Core::IFrameworkView
)]
struct AppSource {
    window: RefCell<Option<CoreWindow>>,
}

#[cfg(feature = "winui-host")]
impl AppSource {
    fn new() -> Self {
        Self {
            window: RefCell::new(None),
        }
    }
}

#[cfg(feature = "winui-host")]
impl IFrameworkViewSource for AppSource {
    fn CreateView(&self) -> Result<IFrameworkView> {
        Ok(self.cast()?)
    }
}

#[cfg(feature = "winui-host")]
impl IFrameworkView for AppSource {
    fn Initialize(&self, _application_view: Option<&IApplicationView>) -> Result<()> {
        Ok(())
    }

    fn SetWindow(&self, window: &CoreWindow) -> Result<()> {
        window.Activate()?;
        *self.window.borrow_mut() = Some(window.clone());
        Ok(())
    }

    fn Load(&self, _entry_point: &HSTRING) -> Result<()> {
        Ok(())
    }

    fn Run(&self) -> Result<()> {
        show_info_box(
            "mdhearts WinUI Host",
            "WinUI integration placeholder running.",
        );
        CoreApplication::Exit();
        Ok(())
    }

    fn Uninitialize(&self) {
        self.window.borrow_mut().take();
    }
}

#[cfg(feature = "winui-host")]
fn show_info_box(title: &str, message: &str) {
    let title_wide = encode_wide(title);
    let message_wide = encode_wide(message);
    unsafe {
        MessageBoxW(
            Some(HWND::default()),
            windows::core::PCWSTR(message_wide.as_ptr()),
            windows::core::PCWSTR(title_wide.as_ptr()),
            MB_ICONINFORMATION | MB_OK,
        );
    }
}

#[cfg(feature = "winui-host")]
fn encode_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
