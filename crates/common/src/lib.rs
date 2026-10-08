pub mod desc;
pub mod diagnostics;
pub mod handler;
pub mod notification;
pub mod speech;
pub mod util;
pub mod worker;

/// Returns the platform's application data directory.
///
/// # Panics
///
/// Panics on Android if the runtime has not initialized its data directory.
#[must_use]
pub fn data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    return ANDROID_DATA_DIR
        .get()
        .expect("Android runtime not initialized")
        .clone();
    #[cfg(not(target_os = "android"))]
    dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
}

#[cfg(target_os = "android")]
pub static ANDROID_DATA_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
