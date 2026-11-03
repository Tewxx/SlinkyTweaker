use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_block_pos_ints",
    pattern: &[
        M("x", "I"),
        Jni(jni::GET_INT),
        M("y", "I"),
        Jni(jni::GET_INT),
        M("z", "I"),
        Jni(jni::GET_INT),
    ],
};
