use crate::automap::{image::Image, Map};

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "push_simple_notify",
        "55 56 48 83 EC 28 48 8D 6C 24 20 4C 8D 0D ?? ?? ?? ??",
    );
}
