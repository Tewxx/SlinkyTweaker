use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_set_current_item",
    pattern: &[M("currentItem", "I"), Jni(jni::SET_INT)],
};
