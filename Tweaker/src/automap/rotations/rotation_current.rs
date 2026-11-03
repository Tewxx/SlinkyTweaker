use crate::automap::{image::Image, Map};

pub(super) fn resolve(image: &Image, map: &mut Map) {
    map.signature(image, "rotation_current", "55 48 89 E5 48 8B 05 ?? ?? ?? ?? 5D C3 0F 1F 00 55 56 48 83 EC 28 48 8D 6C 24 20 48 89 CE E8 ??");
}
