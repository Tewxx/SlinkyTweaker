use super::{image::Image, scan, Map};
use iced_x86::{FlowControl, Mnemonic, OpKind, Register};
use std::collections::BTreeSet;

const WINDOW: usize = 0x400;

pub fn resolve(image: &Image, map: &mut Map) {
    let (Some(host), Some(manager)) = (
        map.anchors.get("module_manager_ref").map(|a| a.rva),
        map.anchors.get("module_manager").map(|a| a.rva),
    ) else {
        return;
    };
    if let Some(set_enabled) = set_enabled(image, host, manager) {
        map.insert("module_set_enabled", set_enabled);
    }
    let Some(&folder) = image.string_hits(".slinky").first() else {
        return;
    };
    let entries: BTreeSet<usize> = string_refs(image, folder)
        .into_iter()
        .filter_map(|at| function_start(image, at))
        .collect();
    if let Some(load) = profile_load(image, host, manager).filter(|f| entries.contains(f)) {
        map.insert("profile_load", load);
    }
    let saves: Vec<usize> = entries.iter().copied().filter(|&f| is_save(image, f)).collect();
    if let [save] = saves[..] {
        map.insert("profile_save", save);
    }
}

fn set_enabled(image: &Image, host: usize, manager: usize) -> Option<usize> {
    let mut at = host;
    let mut loaded = false;
    for _ in 0..64 {
        let inst = scan::instruction(image, at)?;
        if inst.mnemonic() == Mnemonic::Lea
            && inst.is_ip_rel_memory_operand()
            && inst.ip_rel_memory_address() as usize == manager
        {
            loaded = true;
        } else if loaded && inst.mnemonic() == Mnemonic::Call && inst.op0_kind() == OpKind::NearBranch64 {
            return Some(inst.near_branch_target() as usize);
        }
        if matches!(inst.flow_control(), FlowControl::Return) {
            return None;
        }
        at = inst.next_ip() as usize;
    }
    None
}

fn rip_target(image: &Image, at: usize) -> Option<usize> {
    let disp = image.u32(at + 3)? as i32 as isize;
    (at as isize + 7 + disp).try_into().ok()
}

pub(super) fn string_refs(image: &Image, target: usize) -> Vec<usize> {
    let mut hits = Vec::new();
    for section in image.sections.iter().filter(|s| s.executable) {
        let bytes = &image.bytes[section.range.clone()];
        for (i, w) in bytes.windows(3).enumerate() {
            if (w[0] == 0x48 || w[0] == 0x4c) && w[1] == 0x8d && w[2] & 0xc7 == 0x05 {
                let at = section.range.start + i;
                if rip_target(image, at) == Some(target) {
                    hits.push(at);
                }
            }
        }
    }
    hits
}

pub(super) fn function_start(image: &Image, at: usize) -> Option<usize> {
    let lowest = at.saturating_sub(0x100) & !0xf;
    let mut entry = at & !0xf;
    while entry >= lowest {
        if image.read(entry, 1) == Some(&[0x55]) && reaches(image, entry, at) {
            return Some(entry);
        }
        entry = entry.checked_sub(0x10)?;
    }
    None
}

fn reaches(image: &Image, entry: usize, target: usize) -> bool {
    let mut at = entry;
    while at < target {
        let Some(inst) = scan::instruction(image, at) else {
            return false;
        };
        if matches!(inst.mnemonic(), Mnemonic::Int3 | Mnemonic::Ret) {
            return false;
        }
        at = inst.next_ip() as usize;
    }
    at == target
}

fn profile_load(image: &Image, host: usize, manager: usize) -> Option<usize> {
    let start = host.saturating_sub(WINDOW);
    let bytes = image.read(start, host - start)?;
    let found: BTreeSet<usize> = bytes
        .windows(8)
        .enumerate()
        .filter(|(_, w)| w[0] == 0x48 && w[1] == 0x8d && w[2] == 0x15 && w[7] == 0xe8)
        .map(|(i, _)| start + i)
        .filter(|&at| rip_target(image, at) == Some(manager))
        .filter_map(|at| scan::instruction(image, at + 7))
        .filter(|call| call.mnemonic() == Mnemonic::Call && call.op0_kind() == OpKind::NearBranch64)
        .map(|call| call.near_branch_target() as usize)
        .collect();
    (found.len() == 1).then(|| *found.first().unwrap())
}

fn is_save(image: &Image, entry: usize) -> bool {
    let mut at = entry;
    let (mut r8, mut edx, mut tests_rcx) = (false, false, false);
    for _ in 0..12 {
        let Some(inst) = scan::instruction(image, at) else {
            break;
        };
        if inst.mnemonic() == Mnemonic::Mov && inst.op0_kind() == OpKind::Register && inst.op1_kind() == OpKind::Register {
            r8 |= inst.op1_register() == Register::R8;
            edx |= inst.op1_register() == Register::EDX;
        }
        if inst.mnemonic() == Mnemonic::Test && inst.op0_register() == Register::RCX {
            tests_rcx = true;
        }
        if inst.flow_control() != FlowControl::Next {
            break;
        }
        at = inst.next_ip() as usize;
    }
    r8 && edx && !tests_rcx
}
