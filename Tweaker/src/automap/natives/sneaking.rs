use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_sneaking",
    pattern: &[M("isSneaking", "()Z"), Jni(jni::CALL_BOOLEAN)],
};
