use crate::automap::{image::Image, Map};
use iced_x86::Instruction;

pub(super) fn resolve(image: &Image, map: &mut Map, callback: &[Instruction]) {
    super::resolve_callee(
        image,
        map,
        callback,
        "native_angle_magnitude",
        "55 48 89 E5 66 0F 6E C1 48 C1 E9 20 66 0F 6E C9 5D E9 ?? ?? ?? ??",
    );
}
