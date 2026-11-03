use crate::automap::{image::Image, Map};

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "notify_push_signature",
        "2C 00 4B 8D 0C 0A 48 BA CD CC CC CC CC CC CC CC",
    );
}
