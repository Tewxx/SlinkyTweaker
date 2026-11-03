use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_hit_entity",
    pattern: &[
        M("entityHit", "Lnet/minecraft/entity/Entity;"),
        Jni(jni::GET_OBJECT),
    ],
};
