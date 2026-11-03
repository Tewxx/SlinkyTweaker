use super::image::Image;

pub fn matches_at(image: &Image, at: usize, text: &str) -> bool {
    let pattern: Vec<_> = text.split_whitespace().collect();
    if pattern.is_empty() || !image.executable(at) {
        return false;
    }
    let Some(bytes) = image.read(at, pattern.len()) else {
        return false;
    };
    pattern
        .iter()
        .zip(bytes)
        .all(|(token, &byte)| *token == "??" || u8::from_str_radix(token, 16) == Ok(byte))
}
use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic};

pub fn unique(image: &Image, text: &str) -> std::result::Result<usize, String> {
    let pattern: Vec<Option<u8>> = text
        .split_whitespace()
        .map(|t| {
            if t == "??" {
                Ok(None)
            } else {
                u8::from_str_radix(t, 16).map(Some)
            }
        })
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| "invalid signature".to_string())?;
    if pattern.is_empty() || pattern.iter().all(Option::is_none) {
        return Err("empty signature".into());
    }
    let lead = pattern.iter().position(Option::is_some).unwrap();
    let mut found = None;
    for section in image.sections.iter().filter(|s| s.executable) {
        let bytes = &image.bytes[section.range.clone()];
        for (i, &b) in bytes.iter().enumerate() {
            if Some(b) != pattern[lead] || i < lead {
                continue;
            }
            let at = section.range.start + i - lead;
            if at + pattern.len() > section.range.end {
                continue;
            }
            let Some(bytes) = image.read(at, pattern.len()) else {
                continue;
            };
            if !pattern
                .iter()
                .zip(bytes)
                .all(|(p, &b)| p.is_none_or(|p| p == b))
            {
                continue;
            }
            if found.replace(at).is_some() {
                return Err("ambiguous signature (multiple matches)".into());
            }
        }
    }
    found.ok_or_else(|| "signature not found".into())
}
pub fn instruction(image: &Image, at: usize) -> Option<Instruction> {
    if !image.executable(at) {
        return None;
    }
    let end = image
        .sections
        .iter()
        .find(|s| s.executable && s.range.contains(&at))?
        .range
        .end;
    let bytes = (1..=(end - at).min(15))
        .rev()
        .find_map(|len| image.read(at, len))?;
    let mut decoder = Decoder::with_ip(64, bytes, at as u64, DecoderOptions::NONE);
    let inst = decoder.decode();
    (!inst.is_invalid()).then_some(inst)
}
pub fn rip_leas(image: &Image, entry: usize) -> Vec<usize> {
    let mut targets = Vec::new();
    let mut at = entry;
    for _ in 0..512 {
        let Some(inst) = instruction(image, at) else {
            break;
        };
        if inst.mnemonic() == Mnemonic::Lea && inst.is_ip_rel_memory_operand() {
            let target = inst.ip_rel_memory_address() as usize;
            if image.data(target, 8, false) && !targets.contains(&target) {
                targets.push(target)
            }
        }
        at = inst.next_ip() as usize;
        if matches!(
            inst.flow_control(),
            FlowControl::Return | FlowControl::UnconditionalBranch | FlowControl::IndirectBranch
        ) {
            break;
        }
    }
    targets
}

pub fn references(image: &Image, entry: usize) -> Vec<usize> {
    use std::collections::{BTreeSet, VecDeque};
    let mut pending = VecDeque::from([entry]);
    let mut visited = BTreeSet::new();
    let mut refs = BTreeSet::new();
    while let Some(mut at) = pending.pop_front() {
        for _ in 0..2048 {
            if visited.len() >= 4096 || at.abs_diff(entry) > 0x3000 || !visited.insert(at) {
                break;
            }
            let Some(inst) = instruction(image, at) else {
                break;
            };
            if inst.mnemonic() == Mnemonic::Lea && inst.is_ip_rel_memory_operand() {
                refs.insert(inst.ip_rel_memory_address() as usize);
            }
            match inst.flow_control() {
                FlowControl::ConditionalBranch => {
                    pending.push_back(inst.near_branch_target() as usize)
                }
                FlowControl::UnconditionalBranch => {
                    pending.push_back(inst.near_branch_target() as usize);
                    break;
                }
                FlowControl::Return | FlowControl::IndirectBranch | FlowControl::Exception => break,
                _ => (),
            }
            at = inst.next_ip() as usize;
        }
    }
    refs.into_iter().collect()
}

pub fn function_body(image: &Image, entry: usize) -> Vec<iced_x86::Instruction> {
    use iced_x86::FlowControl;
    let mut pending = vec![entry];
    let mut seen = std::collections::BTreeMap::new();
    while let Some(mut at) = pending.pop() {
        for _ in 0..2048 {
            if at.abs_diff(entry) > 0x1800 || seen.contains_key(&at) || seen.len() >= 2048 {
                break;
            }
            let Some(i) = instruction(image, at) else {
                break;
            };
            seen.insert(at, i);
            match i.flow_control() {
                FlowControl::ConditionalBranch => pending.push(i.near_branch_target() as usize),
                FlowControl::UnconditionalBranch => {
                    pending.push(i.near_branch_target() as usize);
                    break;
                }
                FlowControl::Return | FlowControl::IndirectBranch | FlowControl::Exception => break,
                _ => (),
            }
            at = i.next_ip() as usize;
        }
    }
    seen.into_values().collect()
}
