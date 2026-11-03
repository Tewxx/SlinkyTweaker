use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M, Unnamed},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_block_id",
    pattern: &[
        Unnamed,
        M(
            "getIdFromBlock",
            concat!("(", "Lnet/minecraft/", "block/Block;)I"),
        ),
        Jni(jni::CALL_STATIC_INT),
    ],
};
