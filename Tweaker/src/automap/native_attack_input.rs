use super::{image::Image, Map};

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "native_attack_input",
        "55 56 57 48 83 EC 20 48 8D 6C 24 20 0F B6 C2 80 F1 01 0F B6 C9 8D 34 4D F4 8C 00 00 29 C6",
    );
}
