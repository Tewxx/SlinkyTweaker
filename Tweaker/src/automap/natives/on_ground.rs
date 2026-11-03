use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_on_ground",
    pattern: &[M("field_70122_E", "Z"), Jni(jni::GET_BOOLEAN)],
};
