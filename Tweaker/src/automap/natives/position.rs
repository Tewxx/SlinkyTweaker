use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{AnyCall, Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_position",
    pattern: &[
        M("posX", "D"),
        Jni(jni::GET_DOUBLE),
        M("posY", "D"),
        Jni(jni::GET_DOUBLE),
        M("posZ", "D"),
        Jni(jni::GET_DOUBLE),
        AnyCall,
    ],
};
