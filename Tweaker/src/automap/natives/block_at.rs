use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Call, Jni, Tail, Unnamed, Version},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_block_at",
    pattern: &[
        Version,
        Call("native_block_pos_new"),
        Unnamed,
        Jni(jni::CALL_OBJECT),
        Tail("native_get_block"),
    ],
};
