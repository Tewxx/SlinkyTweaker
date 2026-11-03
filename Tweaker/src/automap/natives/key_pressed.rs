use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_key_pressed",
    pattern: &[M("pressed", "Z"), Jni(jni::GET_BOOLEAN)],
};
