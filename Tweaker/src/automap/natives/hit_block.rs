use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Call, Jni, Member as M, Version},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_hit_block",
    pattern: &[
        Version,
        M("blockPos", concat!("Lnet/minecraft/", "util/BlockPos;")),
        Jni(jni::GET_OBJECT),
        Call("native_block_pos_ints"),
    ],
};
