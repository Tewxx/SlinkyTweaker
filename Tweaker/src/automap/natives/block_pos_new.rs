use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M, Unnamed},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_block_pos_new",
    pattern: &[Unnamed, M("<init>", "(III)V"), Jni(jni::NEW_OBJECT)],
};
