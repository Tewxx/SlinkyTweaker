use crate::automap::{image::Image, Map};

pub(super) fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "rotation_claim",
        "55 48 89 E5 8B 05 ?? ?? ?? ?? 39 C1 72 06 89 0D ?? ?? ?? ??",
    );
}
