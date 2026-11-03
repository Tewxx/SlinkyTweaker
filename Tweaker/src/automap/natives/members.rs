use super::abi as jni;
use crate::automap::{image::Image, scan};
use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic, OpKind, Register};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const MCP_189: usize = 4;

#[derive(Debug)]
pub(super) struct Member {
    pub(super) name: String,
    pub(super) srg: String,
    pub(super) signature: String,
}

pub(super) fn members(image: &Image, entries: &mut BTreeSet<usize>) -> BTreeMap<usize, Member> {
    let mut table: BTreeMap<usize, Member> = BTreeMap::new();
    for section in image.sections.iter().filter(|s| s.executable) {
        let Some(bytes) = image.read(section.range.start, section.range.len()) else {
            continue;
        };
        let mut decoder =
            Decoder::with_ip(64, bytes, section.range.start as u64, DecoderOptions::NONE);
        let mut window: Vec<Instruction> = Vec::with_capacity(32);
        for inst in &mut decoder {
            if inst.is_invalid() {
                window.clear();
                continue;
            }
            if matches!(
                inst.flow_control(),
                FlowControl::Call | FlowControl::UnconditionalBranch
            ) && inst.op0_kind() == OpKind::NearBranch64
            {
                let target = inst.near_branch_target() as usize;
                if image.executable(target) {
                    entries.insert(target);
                }
            }
            if inst.flow_control() == FlowControl::IndirectCall
                && inst.op0_kind() == OpKind::Memory
                && !matches!(inst.memory_base(), Register::RIP | Register::None)
                && jni::GET_ID.contains(&(inst.memory_displacement64() as usize))
            {
                if let Some((slot, member)) = lookup(image, &window, &inst) {
                    table.entry(slot).or_insert(member);
                }
            }
            if window.len() == 24 {
                window.remove(0);
            }
            window.push(inst);
        }
    }
    table
}

fn junk_jump(i: &Instruction) -> bool {
    i.flow_control() == FlowControl::UnconditionalBranch
        && (i.next_ip()..=i.next_ip() + 0x20).contains(&i.near_branch_target())
}

fn lookup(image: &Image, window: &[Instruction], call: &Instruction) -> Option<(usize, Member)> {
    let mut table = None;
    let mut literals: HashMap<Register, &str> = HashMap::new();
    for (k, prev) in window.iter().enumerate().rev() {
        if matches!(
            prev.flow_control(),
            FlowControl::Call
                | FlowControl::IndirectCall
                | FlowControl::Return
                | FlowControl::UnconditionalBranch
        ) && !junk_jump(prev)
        {
            break;
        }
        if prev.mnemonic() != Mnemonic::Lea || !prev.is_ip_rel_memory_operand() {
            continue;
        }
        let reg = prev.op0_register();
        let target = prev.ip_rel_memory_address() as usize;
        let indexed = window[k + 1..].iter().take(8).any(|i| {
            i.mnemonic() == Mnemonic::Add
                && i.op0_register() == reg
                && i.op1_kind() == OpKind::Memory
        });
        if indexed {
            table = table.or(Some(target));
        } else if matches!(reg, Register::R8 | Register::R9) {
            if let Some(text) = image.cstr(target) {
                literals.entry(reg).or_insert(text);
            }
        }
    }
    let text = |at: usize| {
        image
            .u64(at)
            .and_then(|p| image.rva(p as usize))
            .and_then(|r| image.cstr(r))
    };
    let (name, srg, signature) = match table {
        Some(t) => (
            text(t + MCP_189 * 0x10)?,
            text(t + (MCP_189 + 1) * 0x10).unwrap_or_default(),
            text(t + MCP_189 * 0x10 + 8)?,
        ),
        None => {
            let name = *literals.get(&Register::R8)?;
            (name, name, *literals.get(&Register::R9)?)
        }
    };
    let mut at = call.next_ip() as usize;
    for _ in 0..8 {
        let inst = scan::instruction(image, at)?;
        if inst.mnemonic() == Mnemonic::Mov
            && inst.op0_kind() == OpKind::Memory
            && inst.op1_kind() == OpKind::Register
            && inst.op1_register() == Register::RAX
            && !matches!(inst.memory_base(), Register::RIP | Register::None)
        {
            let member = Member {
                name: name.into(),
                srg: srg.into(),
                signature: signature.into(),
            };
            return Some((inst.memory_displacement64() as usize, member));
        }
        match inst.flow_control() {
            FlowControl::UnconditionalBranch => at = inst.near_branch_target() as usize,
            FlowControl::Next => at = inst.next_ip() as usize,
            _ => return None,
        }
    }
    None
}
