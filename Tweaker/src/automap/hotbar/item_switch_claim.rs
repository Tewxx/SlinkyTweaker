use super::*;

pub(super) fn owner(image: &Image, body: &[Instruction]) -> Option<usize> {
    if body.len() > 48
        || body.iter().any(|i| {
            matches!(
                i.flow_control(),
                FlowControl::Call | FlowControl::IndirectCall
            )
        })
    {
        return None;
    }
    let stores: BTreeSet<_> = body
        .iter()
        .filter(|i| {
            i.mnemonic() == Mnemonic::Mov
                && writes_global(i)
                && i.op1_kind() == OpKind::Register
                && i.op1_register() == Register::RCX
        })
        .filter_map(global)
        .collect();
    if stores.len() != 1 {
        return None;
    }
    let owner = *stores.first()?;
    if !image.data(owner, 8, true) || owner % 8 != 0 {
        return None;
    }
    let register = body
        .iter()
        .find(|i| {
            i.mnemonic() == Mnemonic::Mov
                && i.op0_kind() == OpKind::Register
                && i.op1_kind() == OpKind::Memory
                && global(i) == Some(owner)
        })?
        .op0_register();
    let compare = body.iter().any(|i| {
        i.mnemonic() == Mnemonic::Cmp
            && i.op0_kind() == OpKind::Register
            && i.op1_kind() == OpKind::Register
            && ((i.op0_register() == register && i.op1_register() == Register::RCX)
                || (i.op1_register() == register && i.op0_register() == Register::RCX))
    });
    let empty = body.iter().any(|i| {
        i.mnemonic() == Mnemonic::Test
            && i.op0_register() == register
            && i.op1_register() == register
    });
    let yes = body.iter().any(|i| {
        i.mnemonic() == Mnemonic::Mov
            && i.op0_register() == Register::AL
            && i.op1_kind() == OpKind::Immediate8
            && i.immediate8() == 1
    });
    let no = body.iter().any(|i| {
        i.mnemonic() == Mnemonic::Xor
            && i.op0_register() == Register::EAX
            && i.op1_register() == Register::EAX
    });
    let branches = body
        .iter()
        .filter(|i| i.flow_control() == FlowControl::ConditionalBranch)
        .count();
    (compare && empty && yes && no && branches >= 2).then_some(owner)
}
