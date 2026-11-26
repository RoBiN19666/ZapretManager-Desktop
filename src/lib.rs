//! Zapret Manager — обход DPI для Discord и YouTube.
//!
//! Полная переписка C# WinForms-приложения на Rust.

pub mod engine;
pub mod ui;
pub mod util;

/// Версия приложения (показывается в интерфейсе и в журнале).
pub const APP_VERSION: &str = "2.0.0";
pub const APP_TITLE: &str = "Zapret Manager";

/// Пути, которые нужны всем частям приложения.
pub mod build_info {
    pub const REPO: &str = "https://github.com/RoBiN19666/ZapretManager-Desktop";
    pub const ZAPRET_REPO: &str = "https://github.com/Flowseal/zapret-discord-youtube";
}
