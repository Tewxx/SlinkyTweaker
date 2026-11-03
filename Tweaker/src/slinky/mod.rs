pub(crate) mod aim;
pub(crate) mod blocks;
pub(crate) mod game;
pub(crate) mod hotbar;
pub(crate) mod layout;
pub(crate) mod mem;
mod notify;
pub(crate) mod place;
pub(crate) mod registration;
pub(crate) mod sections;
pub(crate) mod settings;
pub(crate) mod thunk;

pub use notify::notify_info;

use crate::error::{Error, Result};

pub(crate) fn accent_color() -> [f32; 3] {
    crate::automap::get()
        .ok()
        .and_then(|m| m.address("accent_color_value").ok())
        .map(|a| {
            [
                mem::read_f32(a).unwrap_or(0.4),
                mem::read_f32(a + 4).unwrap_or(0.6),
                mem::read_f32(a + 8).unwrap_or(1.0),
            ]
        })
        .unwrap_or([0.4, 0.6, 1.0])
}

#[derive(Clone, Debug)]
pub struct SlinkyModule {
    pub base: usize,
}

impl SlinkyModule {
    pub fn locate() -> Result<Self> {
        match crate::automap::locate::find_slinky_module() {
            Some((base, size, _)) => {
                crate::automap::initialize(base, size)?;

                Ok(SlinkyModule { base })
            }
            None => {
                Err(Error::NotFound {
                    what: "Slinky module".into(),
                    tried: "no loaded module matched slinky name or signature".into(),
                })
            }
        }
    }

}

