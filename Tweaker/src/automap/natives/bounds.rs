use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{AnyCall, Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_bounds",
    pattern: &[
        M("minX", "D"),
        Jni(jni::GET_DOUBLE),
        M("minY", "D"),
        Jni(jni::GET_DOUBLE),
        M("minZ", "D"),
        Jni(jni::GET_DOUBLE),
        AnyCall,
        M("maxX", "D"),
        Jni(jni::GET_DOUBLE),
        M("maxY", "D"),
        Jni(jni::GET_DOUBLE),
        M("maxZ", "D"),
        Jni(jni::GET_DOUBLE),
        AnyCall,
        AnyCall,
    ],
};
