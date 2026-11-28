//! Zapret Manager — ускорение соединения для Discord и YouTube.
//!
//! Zapret Manager: графическая оболочка для zapret-discord-youtube и TG WS Proxy.

pub mod engine;
pub mod ui;
pub mod util;

/// Версия приложения (показывается в интерфейсе и в журнале).
///
/// Берётся из `Cargo.toml`, а не дублируется строкой: иначе при бампе версии
/// забывается одно из двух мест, и самообновление предлагает установленную
/// версию повторно.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_TITLE: &str = "Zapret Manager";

/// Пути, которые нужны всем частям приложения.
pub mod build_info {
    pub const REPO: &str = "https://github.com/RoBiN19666/ZapretManager-Desktop";
    pub const ZAPRET_REPO: &str = "https://github.com/Flowseal/zapret-discord-youtube";
}
