use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_object_mouse_over",
    pattern: &[
        M(
            "objectMouseOver",
            "Lnet/minecraft/util/MovingObjectPosition;",
        ),
        Jni(jni::GET_OBJECT),
    ],
};
