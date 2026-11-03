use super::abi as jni;
use super::fingerprint::{
    Helper,
    Pattern::{Jni, Member as M},
};

pub(super) const HELPER: Helper = Helper {
    name: "native_entity_id",
    pattern: &[M("field_145783_c", "I"), Jni(jni::GET_INT)],
};
