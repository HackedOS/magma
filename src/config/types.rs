use serde::Deserialize;
use smithay::input::keyboard::{keysyms as KeySyms, xkb, Keysym};

use super::{KeyModifier, KeyModifiers};

#[derive(Deserialize)]
#[serde(transparent)]
pub struct KeyModifiersDef(Vec<KeyModifier>);

impl From<KeyModifiersDef> for KeyModifiers {
    fn from(src: KeyModifiersDef) -> Self {
        src.0.into_iter().fold(
            KeyModifiers {
                ctrl: false,
                alt: false,
                shift: false,
                caps_lock: false,
                logo: false,
                num_lock: false,
            },
            |mut modis, modi: KeyModifier| {
                modis += modi;
                modis
            },
        )
    }
}

#[allow(non_snake_case)]
pub fn deserialize_KeyModifiers<'de, D>(deserializer: D) -> Result<KeyModifiers, D::Error>
where
    D: serde::Deserializer<'de>,
{
    KeyModifiersDef::deserialize(deserializer).map(Into::into)
}

#[allow(non_snake_case)]
pub fn deserialize_Keysym<'de, D>(deserializer: D) -> Result<Keysym, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected};

    let name = String::deserialize(deserializer)?;
    //let name = format!("KEY_{}", code);
    match xkb::keysym_from_name(&name, xkb::KEYSYM_NO_FLAGS) {
        KeySyms::KEY_NoSymbol => match xkb::keysym_from_name(&name, xkb::KEYSYM_CASE_INSENSITIVE) {
            KeySyms::KEY_NoSymbol => Err(<D::Error as Error>::invalid_value(
                Unexpected::Str(&name),
                &"One of the keysym names of xkbcommon.h without the 'KEY_' prefix",
            )),
            x => {
                dbg!(
                    "Key-Binding '{}' only matched case insensitive for {:?}",
                    name,
                    xkb::keysym_get_name(x)
                );
                Ok(x)
            }
        },
        x => Ok(x),
    }
}
