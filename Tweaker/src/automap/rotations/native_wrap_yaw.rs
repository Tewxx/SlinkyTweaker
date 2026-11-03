use crate::automap::{image::Image, Map};
use iced_x86::Instruction;

pub(super) fn resolve(image: &Image, map: &mut Map, callback: &[Instruction]) {
    super::resolve_callee(
        image,
        map,
        callback,
        "native_wrap_yaw",
        "55 48 83 EC 20 48 8D 6C 24 20 F3 0F 10 0D ?? ?? ?? ?? E8 ?? ?? ?? ?? 0F 2E 05 ?? ?? ?? ??",
    );
}
