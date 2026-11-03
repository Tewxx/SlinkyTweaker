use crate::automap::{image::Image, Map};

pub(super) fn resolve(image: &Image, map: &mut Map, entry: usize) {
    let exclude = None;
    super::resolve_getter(
        image,
        map,
        entry,
        "camera_override_active",
        "55 48 89 E5 0F B6 05 ?? ?? ?? ?? 5D C3",
        exclude,
    );
}
