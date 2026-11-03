use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_get_block",
    pattern: &[
        M("getBlock", concat!("()", "Lnet/minecraft/", "block/Block;")),
        Jni(jni::CALL_OBJECT),
    ],
};
