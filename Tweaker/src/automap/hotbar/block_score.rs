use super::*;

pub(super) fn valid(image: &Image, entry: usize) -> bool {
    let body = scan::function_body(image, entry);
    let minus_one = body.iter().any(|i| {
        i.mnemonic() == Mnemonic::Mov
            && i.op0_register() == Register::EAX
            && i.op1_kind() == OpKind::Immediate32
            && i.immediate32() == u32::MAX
    });
    let block_class = body.iter().any(|i| {
        i.mnemonic() == Mnemonic::Mov
            && i.op0_register() == Register::EAX
            && i.op1_kind() == OpKind::Immediate32
            && i.immediate32() == 2
    });
    let input = body.iter().take(12).any(|i| {
        i.mnemonic() == Mnemonic::Lea
            && i.op0_register() == Register::EDX
            && i.memory_base() == Register::RCX
            && i.memory_displacement64() == u64::MAX
    });
    minus_one
        && block_class
        && input
        && !body.iter().any(|i| {
            matches!(
                i.flow_control(),
                FlowControl::Call | FlowControl::IndirectCall
            ) || writes_global(i)
        })
}
