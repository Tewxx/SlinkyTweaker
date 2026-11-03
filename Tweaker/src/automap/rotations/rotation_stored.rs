use crate::automap::{image::Image, Map};

pub(super) fn resolve(image: &Image, map: &mut Map, entry: usize) {
    let exclude = map.anchors.get("rotation_current").map(|a| a.rva);
    super::resolve_getter(
        image,
        map,
        entry,
        "rotation_stored",
        "55 48 89 E5 48 8B 05 ?? ?? ?? ?? 5D C3",
        exclude,
    );
}
