use crate::automap::{image::Image, Map};

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "push_category_notify",
        "55 56 57 48 83 EC 30 48 8D 6C 24 30 8B 41 10 48",
    );
}
