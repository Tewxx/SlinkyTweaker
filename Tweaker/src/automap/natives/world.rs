use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_world",
    pattern: &[
        M(
            "theWorld",
            concat!("Lnet/minecraft/", "client/multiplayer/WorldClient;"),
        ),
        Jni(jni::GET_OBJECT),
    ],
};
