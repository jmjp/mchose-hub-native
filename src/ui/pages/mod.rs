//! Page registry and per-page modules.

pub mod connect;
pub mod effects;
pub mod eq;
pub mod firmware;
pub mod home;
pub mod mic;
pub mod routine;
pub mod settings;
pub mod surround;

/// Application pages (navigation order matters).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Routine,
    Eq,
    Effects,
    Surround,
    Mic,
    Firmware,
    Settings,
}

impl Page {
    pub const fn all() -> &'static [Page] {
        &[
            Page::Home,
            Page::Routine,
            Page::Eq,
            Page::Effects,
            Page::Surround,
            Page::Mic,
            Page::Firmware,
            Page::Settings,
        ]
    }

    pub const fn id(self) -> &'static str {
        match self {
            Page::Home => "home",
            Page::Routine => "routine",
            Page::Eq => "eq",
            Page::Effects => "effects",
            Page::Surround => "surround",
            Page::Mic => "mic",
            Page::Firmware => "firmware",
            Page::Settings => "settings",
        }
    }

    pub fn from_id(id: &str) -> Option<Page> {
        Page::all().iter().copied().find(|page| page.id() == id)
    }

    pub const fn title_key(self) -> &'static str {
        match self {
            Page::Home => "nav.home",
            Page::Routine => "nav.routine",
            Page::Eq => "nav.eq",
            Page::Effects => "nav.effects",
            Page::Surround => "nav.surround",
            Page::Mic => "nav.mic",
            Page::Firmware => "nav.firmware",
            Page::Settings => "nav.settings",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Page::Home => "●",
            Page::Routine => "♪",
            Page::Eq => "≡",
            Page::Effects => "◆",
            Page::Surround => "◎",
            Page::Mic => "○",
            Page::Firmware => "▲",
            Page::Settings => "⚙",
        }
    }
}
