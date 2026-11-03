use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{AnyCall, Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_prev_position",
    pattern: &[
        M("prevPosX", "D"),
        Jni(jni::GET_DOUBLE),
        M("prevPosY", "D"),
        Jni(jni::GET_DOUBLE),
        M("prevPosZ", "D"),
        Jni(jni::GET_DOUBLE),
        AnyCall,
    ],
};
