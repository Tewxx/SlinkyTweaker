use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_focus",
    pattern: &[M("inGameHasFocus", "Z"), Jni(jni::GET_BOOLEAN)],
};
