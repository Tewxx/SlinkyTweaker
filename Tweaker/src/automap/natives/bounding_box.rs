use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_bounding_box",
    pattern: &[
        M("boundingBox", "Lnet/minecraft/util/AxisAlignedBB;"),
        Jni(jni::GET_OBJECT),
    ],
};
