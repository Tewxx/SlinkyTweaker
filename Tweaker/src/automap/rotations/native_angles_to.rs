use crate::automap::{image::Image, Map};
use iced_x86::Instruction;

pub(super) fn resolve(image: &Image, map: &mut Map, callback: &[Instruction]) {
    super::resolve_callee(image, map, callback, "native_angles_to", "55 48 83 EC 60 48 8D 6C 24 60 44 0F 29 4D F0 44 0F 29 45 E0 0F 29 7D D0 0F 29 75 C0 F2 0F 10 0A F2 0F 5C 09");
}
