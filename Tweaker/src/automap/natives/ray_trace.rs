use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_ray_trace",
    pattern: &[
        M(
            "rayTraceBlocks",
            concat!(
                "(",
                "Lnet/minecraft/",
                "util/Vec3;",
                "Lnet/minecraft/",
                "util/Vec3;ZZZ)",
                "Lnet/minecraft/",
                "util/MovingObjectPosition;"
            ),
        ),
        Jni(jni::CALL_OBJECT),
    ],
};
