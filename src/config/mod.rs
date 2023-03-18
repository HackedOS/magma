use std::{collections::HashMap, fs::OpenOptions};

use serde::Deserialize;
use smithay::input::keyboard::ModifiersState;

use self::types::{deserialize_KeyModifiers, deserialize_Keysym};

mod types;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub workspaces: u8,
    pub keybindings: HashMap<KeyPattern, Action>,

    #[serde(default = "default_gaps")]
    pub gaps: (u8, u8),
}
impl Config {
    pub fn load() -> Config {
        let xdg = xdg::BaseDirectories::new().ok();
        let locations = if let Some(base) = xdg {
            vec![
                base.get_config_file("holowm.ron"),
                base.get_config_file("holowm/config.ron"),
            ]
        } else {
            Vec::with_capacity(3)
        };

        for path in locations {
            println!("Trying config location: {}", path.display());
            if path.exists() {
                println!("Using config at {}", path.display());
                return ron::de::from_reader(OpenOptions::new().read(true).open(path).unwrap())
                    .expect("Malformed config file");
            }
        }
        Config {
            workspaces: 1,
            keybindings: HashMap::new(),
            gaps: default_gaps(),
        }
    }
}

fn default_gaps() -> (u8, u8) {
    (0, 4)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum KeyModifier {
    Ctrl,
    Alt,
    Shift,
    Super,
    CapsLock,
    NumLock,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyModifiers {
    ctrl: bool,
    alt: bool,
    shift: bool,
    logo: bool,
    caps_lock: bool,
    num_lock: bool,
}

impl PartialEq<ModifiersState> for KeyModifiers {
    fn eq(&self, other: &ModifiersState) -> bool {
        self.ctrl == other.ctrl
            && self.alt == other.alt
            && self.shift == other.shift
            && self.logo == other.logo
            && self.caps_lock == other.caps_lock
            && self.num_lock == other.num_lock
    }
}

impl std::ops::AddAssign<KeyModifier> for KeyModifiers {
    fn add_assign(&mut self, rhs: KeyModifier) {
        match rhs {
            KeyModifier::Ctrl => self.ctrl = true,
            KeyModifier::Alt => self.alt = true,
            KeyModifier::Shift => self.shift = true,
            KeyModifier::Super => self.logo = true,
            KeyModifier::CapsLock => self.caps_lock = true,
            KeyModifier::NumLock => self.num_lock = true,
        };
    }
}

impl std::ops::BitOr for KeyModifier {
    type Output = KeyModifiers;

    fn bitor(self, rhs: KeyModifier) -> Self::Output {
        let mut modifiers = self.into();
        modifiers += rhs;
        modifiers
    }
}

impl Into<KeyModifiers> for KeyModifier {
    fn into(self) -> KeyModifiers {
        let mut modifiers = KeyModifiers {
            ctrl: false,
            alt: false,
            shift: false,
            caps_lock: false,
            logo: false,
            num_lock: false,
        };
        modifiers += self;
        modifiers
    }
}

/// Describtion of a key combination that might be
/// handled by the compositor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Hash)]
#[serde(deny_unknown_fields)]
pub struct KeyPattern {
    /// What modifiers are expected to be pressed alongside the key
    #[serde(deserialize_with = "deserialize_KeyModifiers")]
    pub modifiers: KeyModifiers,
    /// The actual key, that was pressed
    #[serde(deserialize_with = "deserialize_Keysym")]
    pub key: u32,
}

impl KeyPattern {
    pub fn new(modifiers: impl Into<KeyModifiers>, key: u32) -> KeyPattern {
        KeyPattern {
            modifiers: modifiers.into(),
            key,
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub enum Action {
    Terminate,
    Debug,
    Close,

    Workspace(u8),
    ToggleWindowFloating,

    Spawn(String),
}
