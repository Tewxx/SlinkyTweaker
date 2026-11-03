use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_stack_size",
    pattern: &[M("stackSize", "I"), Jni(jni::GET_INT)],
};
