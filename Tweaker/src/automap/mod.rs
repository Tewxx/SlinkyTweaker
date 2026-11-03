pub mod accent_color;
pub mod block_in;
pub mod catalog;
pub mod displace;
pub mod events;
pub mod font;
pub mod format_text;
pub mod gui;
pub mod hotbar;
pub mod image;
pub mod jni;
pub(crate) mod locate;
mod model;
pub mod module_manager;
pub mod native_attack_input;
pub mod natives;
pub mod notifications;
pub mod rotations;
pub mod scan;
pub mod settings;
pub mod silent_aim;
mod sync_calls;
pub(crate) mod text_box;
pub(crate) mod ui_context;

use crate::{Error, Result};
pub use events::Event;
use image::Image;
pub use model::{Hook, Map, Module, Setting};
use std::sync::OnceLock;

static MAPPING: OnceLock<Map> = OnceLock::new();

pub fn resolve(image: &Image) -> Result<Map> {
    if !image.is_slinky() {
        return Err(Error::Jvm("automap: image identity check failed".into()));
    }
    let mut map = Map {
        base: image.base,
        image_size: image.bytes.len(),
        ..Map::default()
    };
    catalog::resolve(image, &mut map);
    module_manager::resolve(image, &mut map);
    if map.modules.len() < 10 {
        return Err(Error::Jvm(
            "automap: module descriptors not initialized yet".into(),
        ));
    }
    events::resolve(&mut map);
    block_in::resolve(&mut map);
    settings::resolve(image, &mut map);
    gui::resolve(image, &mut map);
    text_box::resolve(image, &mut map);
    accent_color::resolve(&mut map);
    rotations::resolve(image, &mut map);
    silent_aim::resolve(image, &mut map);
    notifications::resolve(image, &mut map);
    jni::resolve(image, &mut map);
    natives::resolve(image, &mut map);
    displace::resolve(image, &mut map);
    hotbar::resolve(image, &mut map);
    font::resolve(image, &mut map);
    Ok(map)
}

pub fn initialize(base: usize, size: usize) -> Result<()> {
    if let Some(map) = MAPPING.get() {
        return if map.base == base {
            Ok(())
        } else {
            Err(Error::Jvm(
                "automap already belongs to another image".into(),
            ))
        };
    }
    let image = Image::capture(base, size)?;
    let map = resolve(&image)?;
    map.require(&["module_manager"])?;
    module_manager::validate_live(&map)?;
    MAPPING
        .set(map)
        .map_err(|_| Error::Jvm("automap initialized concurrently".into()))
}

pub fn get() -> Result<&'static Map> {
    MAPPING
        .get()
        .ok_or_else(|| Error::Jvm("automap not initialized".into()))
}
pub fn require(names: &[&str]) -> Result<()> {
    get()?.require(names)
}

pub(crate) fn rva(name: &str) -> usize {
    get()
        .expect("feature used before automap initialization")
        .anchors
        .get(name)
        .expect("feature used without checking required anchors")
        .rva
}
