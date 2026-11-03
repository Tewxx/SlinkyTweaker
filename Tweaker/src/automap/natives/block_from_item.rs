use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M, Unnamed},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_block_from_item",
    pattern: &[
        Unnamed,
        M(
            "getBlockFromItem",
            concat!(
                "(",
                "Lnet/minecraft/",
                "item/Item;)",
                "Lnet/minecraft/",
                "block/Block;"
            ),
        ),
        Jni(jni::CALL_STATIC_OBJECT),
    ],
};
