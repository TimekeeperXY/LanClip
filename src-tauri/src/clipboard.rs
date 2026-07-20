mod common;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub use macos::SystemClipboard;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use unsupported::SystemClipboard;
#[cfg(target_os = "windows")]
pub use windows::SystemClipboard;

pub use common::thumbnail_data_url;
