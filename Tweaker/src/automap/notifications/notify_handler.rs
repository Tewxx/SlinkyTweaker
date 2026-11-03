use crate::automap::{image::Image, Map};

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "notify_handler",
        "48 83 EC 28 48 8D 6C 24 20 48 8B 41 10 48 8D 15",
    );
}
