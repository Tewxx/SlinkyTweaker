use crate::automap::{image::Image, Map};

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "format_text",
        "55 56 57 53 48 83 EC 38 48 8D 6C 24 30 4C 89 C6",
    );
}
