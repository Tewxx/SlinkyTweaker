mod block_score;
mod block_select;
mod block_select_release;
mod item_switch_claim;
mod item_switch_release;

use super::{image::Image, scan, Map};
use iced_x86::{FlowControl, Instruction, Mnemonic, OpKind, Register};
use std::collections::{BTreeMap, BTreeSet};

pub fn resolve(image: &Image, map: &mut Map) {
    let mut bodies = BTreeMap::new();
    let mut witnesses: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
    for name in [
        "Auto Rod",
        "Auto Pot",
        "Auto Soup",
        "Auto Tool",
        "Auto Weapon",
        "Block In",
        "Bridge Assist",
        "Clutch",
    ] {
        let Some(module) = map.modules.get(name) else {
            continue;
        };
        let mut pending: Vec<_> = module.hooks.iter().map(|h| (h.callback, 0)).collect();
        let mut seen = BTreeSet::new();
        while let Some((entry, depth)) = pending.pop() {
            if seen.len() >= 384 || !seen.insert(entry) || !image.executable(entry) {
                continue;
            }
            witnesses.entry(entry).or_default().insert(name.to_string());
            let body = bodies
                .entry(entry)
                .or_insert_with(|| scan::function_body(image, entry));
            if depth < 2 {
                pending.extend(
                    body.iter()
                        .filter(|i| {
                            i.flow_control() == FlowControl::Call
                                && i.op0_kind() == OpKind::NearBranch64
                        })
                        .map(|i| (i.near_branch_target() as usize, depth + 1)),
                );
                if let Some(target) = thunk_target(image, entry) {
                    pending.push((target, depth + 1));
                }
            }
        }
    }
    let claims: Vec<_> = bodies
        .iter()
        .filter_map(|(&entry, body)| {
            if witnesses.get(&entry)?.len() < 2 {
                return None;
            }
            Some((entry, item_switch_claim::owner(image, body)?))
        })
        .collect();
    let releases: Vec<_> = bodies
        .iter()
        .filter_map(|(&entry, body)| {
            if witnesses.get(&entry)?.len() < 2 {
                return None;
            }
            Some((entry, item_switch_release::owner(image, body)?))
        })
        .collect();
    let pairs: Vec<_> = claims
        .iter()
        .flat_map(|&(claim, owner)| {
            releases
                .iter()
                .filter(move |(_, candidate)| *candidate == owner)
                .map(move |&(release, _)| (claim, release, owner))
        })
        .collect();
    let [(claim, release, owner)] = pairs.as_slice() else {
        return;
    };
    for (name, rva) in [
        ("item_switch_claim", *claim),
        ("item_switch_release", *release),
        ("item_switch_owner", *owner),
    ] {
        map.insert(name, rva);
    }
    block_select::resolve(image, map, &bodies, &witnesses, *claim);
    block_select_release::resolve(image, map, &bodies, *release);
}

fn thunk_target(image: &Image, entry: usize) -> Option<usize> {
    let mut at = entry;
    for _ in 0..12 {
        let i = scan::instruction(image, at)?;
        if i.flow_control() == FlowControl::UnconditionalBranch {
            return image
                .executable(i.near_branch_target() as usize)
                .then_some(i.near_branch_target() as usize);
        }
        if i.flow_control() != FlowControl::Next {
            return None;
        }
        at = i.next_ip() as usize;
    }
    None
}

fn global(i: &Instruction) -> Option<usize> {
    i.is_ip_rel_memory_operand()
        .then_some(i.ip_rel_memory_address() as usize)
}
fn writes_global(i: &Instruction) -> bool {
    i.op0_kind() == OpKind::Memory && i.is_ip_rel_memory_operand()
}
fn calls(body: &[Instruction], target: usize) -> bool {
    body.iter().any(|i| {
        i.flow_control() == FlowControl::Call
            && i.op0_kind() == OpKind::NearBranch64
            && i.near_branch_target() as usize == target
    })
}

fn small_functions(image: &Image, max_len: usize) -> Vec<usize> {
    let Some(pe) = image.u32(0x3c).map(|v| v as usize) else {
        return Vec::new();
    };
    let directory = pe + 24 + 0x70 + 3 * 8;
    let (Some(start), Some(size)) = (image.u32(directory), image.u32(directory + 4)) else {
        return Vec::new();
    };
    let (start, size) = (start as usize, size as usize);
    if size % 12 != 0 || size > 12 * 500_000 || image.read(start, size).is_none() {
        return Vec::new();
    }
    (start..start + size)
        .step_by(12)
        .filter_map(|at| {
            let (begin, end) = (image.u32(at)? as usize, image.u32(at + 4)? as usize);
            (end > begin
                && end - begin <= max_len
                && image.executable(begin)
                && image.executable(end - 1))
            .then_some(begin)
        })
        .collect()
}
