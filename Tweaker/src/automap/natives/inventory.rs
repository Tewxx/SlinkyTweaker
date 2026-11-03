use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_inventory",
    pattern: &[
        M(
            "inventory",
            concat!("Lnet/minecraft/", "entity/player/InventoryPlayer;"),
        ),
        Jni(jni::GET_OBJECT),
    ],
};
