use super::{image::Image, scan, Map};
use iced_x86::{FlowControl, Instruction, Mnemonic, OpKind, Register};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug)]
pub struct TextBox {
    pub state: usize,
    pub text: usize,
    pub target: usize,
}

pub fn resolve(image: &Image, map: &mut Map) {
    let Some(gui_float) = map.anchors.get("gui_float").map(|a| a.rva) else {
        return;
    };
    let body = head(image, gui_float, 40);
    let widget = body.iter().find(|i| i.flow_control() == FlowControl::Call).map(|i| i.near_branch_target() as usize);
    let changed = body
        .iter()
        .position(|i| {
            i.mnemonic() == Mnemonic::Mov
                && i.op0_register() == Register::CL
                && i.op1_kind() == OpKind::Immediate8
                && i.immediate8() == 1
        })
        .and_then(|set| {
            body[set + 1..]
                .iter()
                .take(4)
                .find(|i| matches!(i.flow_control(), FlowControl::Call | FlowControl::UnconditionalBranch))
        })
        .map(|i| i.near_branch_target() as usize);
    if let Some(rva) = changed {
        map.insert("settings_changed", rva);
    }
    let Some(widget) = widget else {
        return;
    };
    let mut functions = BTreeSet::new();
    for f in callees(image, widget) {
        functions.insert(f);
        functions.extend(callees(image, f));
    }
    let checks: BTreeSet<_> = functions.iter().filter_map(|&f| editing_check(image, f)).collect();
    let getters: BTreeSet<_> = functions.iter().filter_map(|&f| text_getter(image, f)).collect();
    if (checks.len(), getters.len()) == (1, 1) {
        let (state, target) = *checks.first().unwrap();
        let (getter_state, text) = *getters.first().unwrap();
        if state == getter_state {
            map.text_box = Some(TextBox { state, text, target });
        }
    }
}

fn callees(image: &Image, entry: usize) -> BTreeSet<usize> {
    scan::function_body(image, entry)
        .iter()
        .filter(|i| i.flow_control() == FlowControl::Call && i.near_branch_target() != 0)
        .map(|i| i.near_branch_target() as usize)
        .collect()
}

fn head(image: &Image, entry: usize, count: usize) -> Vec<Instruction> {
    let mut out = Vec::new();
    let mut at = entry;
    while out.len() < count + 2 {
        let Some(i) = scan::instruction(image, at) else { break };
        at = i.next_ip() as usize;
        out.push(i);
    }
    let prologue = |i: &Instruction| {
        (i.mnemonic() == Mnemonic::Push && i.op0_register() == Register::RBP)
            || (i.mnemonic() == Mnemonic::Mov && i.op0_register() == Register::RBP)
    };
    out.into_iter().skip_while(prologue).take(count).collect()
}

fn imm(i: &Instruction) -> Option<u64> {
    let immediate = i.op_count() > 1
        && matches!(
            i.op1_kind(),
            OpKind::Immediate8 | OpKind::Immediate8to32 | OpKind::Immediate8to64 | OpKind::Immediate32 | OpKind::Immediate32to64
        );
    immediate.then(|| i.immediate(1))
}

fn rcx_offset(i: &Instruction) -> Option<usize> {
    (i.memory_base() == Register::RCX && i.memory_index() == Register::None)
        .then(|| i.memory_displacement64() as usize)
}

fn state_test(h: &[Instruction]) -> Option<usize> {
    let [load, and, cmp, jne, ..] = h else { return None };
    let ok = load.mnemonic() == Mnemonic::Mov
        && load.op0_register() == Register::EAX
        && and.mnemonic() == Mnemonic::And
        && imm(and).map(|v| v as u32) == Some(0xffff_fffe)
        && cmp.mnemonic() == Mnemonic::Cmp
        && cmp.op0_register() == Register::EAX
        && imm(cmp) == Some(2)
        && jne.mnemonic() == Mnemonic::Jne;
    if ok { rcx_offset(load) } else { None }
}

fn editing_check(image: &Image, entry: usize) -> Option<(usize, usize)> {
    let h = head(image, entry, 6);
    let state = state_test(&h)?;
    let (cmp, sete) = (h.get(4)?, h.get(5)?);
    let ok = cmp.mnemonic() == Mnemonic::Cmp
        && cmp.op1_register() == Register::RDX
        && sete.mnemonic() == Mnemonic::Sete;
    if ok { Some((state, rcx_offset(cmp)?)) } else { None }
}

fn text_getter(image: &Image, entry: usize) -> Option<(usize, usize)> {
    let h = head(image, entry, 7);
    let state = state_test(&h)?;
    let (cmp, jne, lea) = (h.get(4)?, h.get(5)?, h.get(6)?);
    let ok = cmp.mnemonic() == Mnemonic::Cmp
        && imm(cmp) == Some(1)
        && rcx_offset(cmp).is_some()
        && jne.mnemonic() == Mnemonic::Jne
        && lea.mnemonic() == Mnemonic::Lea
        && lea.op0_register() == Register::RAX;
    if ok { Some((state, rcx_offset(lea)?)) } else { None }
}
