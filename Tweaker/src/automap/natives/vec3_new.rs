use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M, Unnamed},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_vec3_new",
    pattern: &[Unnamed, M("<init>", "(DDD)V"), Jni(jni::NEW_OBJECT)],
};
