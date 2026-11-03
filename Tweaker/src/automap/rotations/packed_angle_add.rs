use crate::automap::{image::Image, Map};

pub(super) fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "packed_angle_add",
        "55 48 89 E5 66 0F 6E C1 48 C1 E9 20 66 0F 6E C9 66 0F 6E D2 F3 0F 58 D0",
    );
}
