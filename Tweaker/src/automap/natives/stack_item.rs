use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_stack_item",
    pattern: &[
        M("getItem", concat!("()", "Lnet/minecraft/", "item/Item;")),
        Jni(jni::CALL_OBJECT),
    ],
};
