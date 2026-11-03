use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Tail, Unnamed, Version},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_hit_side",
    pattern: &[
        Version,
        Unnamed,
        Jni(jni::GET_OBJECT),
        Tail("native_ordinal"),
    ],
};
