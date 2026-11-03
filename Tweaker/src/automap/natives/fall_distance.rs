use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_fall_distance",
    pattern: &[M("fallDistance", "F"), Jni(jni::GET_FLOAT)],
};
