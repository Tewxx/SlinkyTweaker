use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_ordinal",
    pattern: &[M("ordinal", "()I"), Jni(jni::CALL_INT)],
};
