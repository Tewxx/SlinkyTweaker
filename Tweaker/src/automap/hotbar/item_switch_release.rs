use super::*;

pub(super) fn owner(image: &Image, body: &[Instruction]) -> Option<usize> {
    if body.len() > 40
        || body.iter().any(|i| {
            matches!(
                i.flow_control(),
                FlowControl::Call | FlowControl::IndirectCall
            )
        })
    {
        return None;
    }
    let comparisons: BTreeSet<_> = body
        .iter()
        .filter(|i| {
            i.mnemonic() == Mnemonic::Cmp
                && i.op0_kind() == OpKind::Memory
                && i.op1_kind() == OpKind::Register
                && i.op1_register() == Register::RCX
        })
        .filter_map(global)
        .collect();
    if comparisons.len() != 1 {
        return None;
    }
    let owner = *comparisons.first()?;
    if !image.data(owner, 8, true) || owner % 8 != 0 {
        return None;
    }
    let clear = body.iter().any(|i| {
        i.mnemonic() == Mnemonic::Mov
            && writes_global(i)
            && global(i) == Some(owner)
            && i.op1_kind() == OpKind::Immediate32to64
            && i.immediate32to64() == 0
    });
    let only_owner = body
        .iter()
        .filter(|i| writes_global(i) && i.mnemonic() != Mnemonic::Cmp)
        .all(|i| global(i) == Some(owner));
    (clear
        && only_owner
        && body
            .iter()
            .any(|i| i.flow_control() == FlowControl::ConditionalBranch))
    .then_some(owner)
}
