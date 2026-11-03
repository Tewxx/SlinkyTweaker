use super::{members::Member, trace::Step};
use crate::automap::Map;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) enum Pattern {
    Version,
    Member(&'static str, &'static str),
    Unnamed,
    Jni(usize),
    Call(&'static str),
    AnyCall,
    Tail(&'static str),
}
use Pattern::{AnyCall, Call, Jni, Member as M, Tail, Unnamed, Version};

pub(super) struct Helper {
    pub name: &'static str,
    pub pattern: &'static [Pattern],
}

pub(super) fn matches(
    steps: &[Step],
    pattern: &[Pattern],
    members: &BTreeMap<usize, Member>,
    map: &Map,
) -> bool {
    let anchor = |name: &str| map.anchors.get(name).map(|a| a.rva);
    steps.len() == pattern.len()
        && steps
            .iter()
            .zip(pattern)
            .all(|(step, pattern)| match (*step, *pattern) {
                (Step::Version, Version) => true,
                (Step::Slot(slot), M(name, signature)) => members
                    .get(&slot)
                    .is_some_and(|m| (m.name == name || m.srg == name) && m.signature == signature),
                (Step::Slot(slot), Unnamed) => !members.contains_key(&slot),
                (Step::Jni(a), Jni(b)) => a == b,
                (Step::Call(at), Call(name)) | (Step::Tail(at), Tail(name)) => {
                    anchor(name) == Some(at)
                }
                (Step::Call(_) | Step::Tail(_), AnyCall) => true,
                _ => false,
            })
}
