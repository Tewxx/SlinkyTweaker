use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_stack_in_slot",
    pattern: &[
        M(
            "getStackInSlot",
            concat!("(I)", "Lnet/minecraft/", "item/ItemStack;"),
        ),
        Jni(jni::CALL_OBJECT),
    ],
};
