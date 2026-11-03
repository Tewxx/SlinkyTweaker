use super::{image::Image, Map};

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "jni_font_draw",
        concat!(
            "55 41 57 41 56 41 55 41 54 56 57 53 48 83 EC 58 ",
            "48 8D 6C 24 50 0F 29 75 F0 4C 89 C6 48 89 CF ",
            "44 0F B6 BD 80 00 00 00 44 8B 75 78 F3 0F 10 75 70 ",
            "4C 8B 21 41 83 3C 24 00 74 3C 48 8B 4F 08 48 8B 01 ",
            "F3 0F 5A C3 4D 8B 84 24 ?? ?? ?? ?? F3 0F 5A CE ",
            "45 0F B6 CF 44 89 4C 24 38 44 89 74 24 30 ",
            "F2 0F 11 4C 24 28 F2 0F 11 44 24 20 49 89 F1 FF 90 88 01 00 00"
        ),
    );
}
