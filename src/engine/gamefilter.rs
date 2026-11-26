//! Состояние игрового фильтра: utils\game_filter.enabled.

use std::path::Path;

use crate::util::is_port_range_valid;

use super::textfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameFilterMode {
    Disabled,
    All,
    Tcp,
    Udp,
}

impl GameFilterMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameFilterMode::Disabled => "disabled",
            GameFilterMode::All => "all",
            GameFilterMode::Tcp => "tcp",
            GameFilterMode::Udp => "udp",
        }
    }

    pub fn from_str(s: &str) -> GameFilterMode {
        match s.trim().to_lowercase().as_str() {
            "all" => GameFilterMode::All,
            "tcp" => GameFilterMode::Tcp,
            "udp" => GameFilterMode::Udp,
            _ => GameFilterMode::Disabled,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            GameFilterMode::Disabled => "выключен",
            GameFilterMode::All => "включён (TCP и UDP)",
            GameFilterMode::Tcp => "включён (только TCP)",
            GameFilterMode::Udp => "включён (только UDP)",
        }
    }

    pub fn is_on(&self) -> bool {
        *self != GameFilterMode::Disabled
    }
}

#[derive(Debug, Clone)]
pub struct GameFilterState {
    pub mode: GameFilterMode,
    pub tcp_range: String,
    pub udp_range: String,
}

impl Default for GameFilterState {
    fn default() -> Self {
        GameFilterState {
            mode: GameFilterMode::Disabled,
            tcp_range: "1024-65535".to_string(),
            udp_range: "1024-65535".to_string(),
        }
    }
}

impl GameFilterState {
    pub fn tcp_token(&self) -> String {
        if matches!(self.mode, GameFilterMode::All | GameFilterMode::Tcp) {
            self.tcp_range.clone()
        } else {
            "12".to_string()
        }
    }

    pub fn udp_token(&self) -> String {
        if matches!(self.mode, GameFilterMode::All | GameFilterMode::Udp) {
            self.udp_range.clone()
        } else {
            "12".to_string()
        }
    }

    /// Общий токен %GameFilter% для старых батников.
    pub fn filter_token(&self) -> String {
        match self.mode {
            GameFilterMode::All => self.tcp_range.clone(),
            GameFilterMode::Tcp => self.tcp_range.clone(),
            GameFilterMode::Udp => self.udp_range.clone(),
            GameFilterMode::Disabled => "12".to_string(),
        }
    }

    pub fn mode_text(&self) -> &'static str {
        self.mode.label()
    }

    pub fn load(root: &str) -> GameFilterState {
        let mut st = GameFilterState::default();
        let file = Path::new(root).join("utils").join("game_filter.enabled");
        if !file.is_file() {
            return st;
        }
        for raw in std::fs::read_to_string(&file)
            .unwrap_or_default()
            .lines()
        {
            let line = raw.trim();
            let Some(eq) = line.find('=') else { continue };
            let key = line[..eq].trim().to_lowercase();
            let value = line[eq + 1..].trim().to_string();
            match key.as_str() {
                "mode" => {
                    if !value.is_empty() {
                        st.mode = GameFilterMode::from_str(&value);
                    }
                }
                "all" => st.mode = GameFilterMode::All,
                "tcp" => {
                    if value.is_empty() {
                        st.mode = GameFilterMode::Tcp;
                    } else {
                        st.tcp_range = value;
                    }
                }
                "udp" => {
                    if value.is_empty() {
                        st.mode = GameFilterMode::Udp;
                    } else {
                        st.udp_range = value;
                    }
                }
                _ => {}
            }
        }
        if !is_port_range_valid(&st.tcp_range) {
            st.tcp_range = "1024-65535".to_string();
        }
        if !is_port_range_valid(&st.udp_range) {
            st.udp_range = "1024-65535".to_string();
        }
        st
    }

    pub fn save(&self, root: &str) -> std::io::Result<()> {
        let file = Path::new(root).join("utils").join("game_filter.enabled");
        let text = format!(
            "mode={}\r\ntcp={}\r\nudp={}\r\n",
            self.mode.as_str(),
            self.tcp_range,
            self.udp_range
        );
        textfile::write_text_atomic(&file, &text)
    }
}
