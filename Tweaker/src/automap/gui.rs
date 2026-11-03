use super::{image::Image, scan, Map};
use iced_x86::{FlowControl, Instruction, Mnemonic, Register};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub fn resolve(image: &Image, map: &mut Map) {
    let descriptors: BTreeMap<_, _> = map
        .modules
        .values()
        .flat_map(|m| m.settings.values())
        .filter(|s| s.kind != "unknown")
        .map(|s| (s.descriptor, s.kind.clone()))
        .collect();
    let sync: BTreeSet<_> = map
        .anchors
        .iter()
        .filter(|(n, _)| n.starts_with("sync_"))
        .map(|(_, a)| a.rva)
        .collect();
    let mut targets: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for module in map.modules.values() {
        let Some(entry) = module.gui_callback else {
            continue;
        };
        let mut queue = VecDeque::from([(entry, None)]);
        let mut seen = BTreeSet::new();
        while let Some((mut at, mut descriptor)) = queue.pop_front() {
            for _ in 0..4096 {
                if seen.len() >= 8192
                    || at.abs_diff(entry) > 0x10000
                    || !seen.insert((at, descriptor))
                {
                    break;
                }
                let Some(inst) = scan::instruction(image, at) else {
                    break;
                };
                if inst.mnemonic() == Mnemonic::Lea && inst.is_ip_rel_memory_operand() {
                    let target = inst.ip_rel_memory_address() as usize;
                    if descriptors.contains_key(&target) {
                        descriptor = Some(target);
                    }
                }
                match inst.flow_control() {
                    FlowControl::Call | FlowControl::UnconditionalBranch => {
                        let target = inst.near_branch_target() as usize;
                        if inst.flow_control() == FlowControl::UnconditionalBranch
                            && target >= entry
                            && target.abs_diff(entry) <= 0x10000
                        {
                            queue.push_back((target, descriptor));
                            break;
                        }
                        if let Some(descriptor) = descriptor {
                            if image.executable(target) && !sync.contains(&target) {
                                targets.entry(descriptor).or_default().insert(target);
                            }
                        }
                        if inst.flow_control() == FlowControl::UnconditionalBranch {
                            break;
                        }
                        descriptor = None;
                    }
                    FlowControl::ConditionalBranch => {
                        queue.push_back((inst.near_branch_target() as usize, descriptor))
                    }
                    FlowControl::Return | FlowControl::IndirectBranch | FlowControl::Exception => {
                        break
                    }
                    FlowControl::IndirectCall => descriptor = None,
                    _ => (),
                }
                at = inst.next_ip() as usize;
            }
        }
    }
    let mut kinds: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (&descriptor, candidates) in &targets {
        if candidates.len() == 1 {
            kinds
                .entry(descriptors[&descriptor].clone())
                .or_default()
                .insert(*candidates.first().unwrap());
        }
    }
    for (kind, candidates) in kinds {
        if candidates.len() == 1 {
            let helper = *candidates.first().unwrap();
            map.insert(&format!("gui_{kind}"), helper);
            for setting in map
                .modules
                .values_mut()
                .flat_map(|m| m.settings.values_mut())
                .filter(|s| s.kind == kind)
            {
                if targets
                    .get(&setting.descriptor)
                    .is_some_and(|t| t.len() == 1 && t.contains(&helper))
                {
                    setting.gui_helper = Some(helper);
                }
            }
        }
    }
    resolve_section(image, map);
    resolve_layout_next(image, map);
    if let Some(gui) = map.modules.get("GUI") {
        let (descriptor, render, sync) = (gui.descriptor, gui.gui_callback, gui.sync_callback);
        map.insert("gui_module", descriptor);
        if let Some(rva) = render {
            map.insert("gui_settings_callback", rva);
        }
        if let Some(rva) = sync {
            map.insert("gui_sync_callback", rva);
        }
    }
}

fn resolve_layout_next(image: &Image, map: &mut Map) {
    let first_call = |body: &[Instruction]| {
        let at = body.iter().position(|i| i.flow_control() == FlowControl::Call)?;
        Some((at, body[at].near_branch_target() as usize))
    };
    let next = map.anchors.get("gui_bool").and_then(|gui_bool| {
        let (_, widget) = first_call(&scan::function_body(image, gui_bool.rva))?;
        let body = scan::function_body(image, widget);
        let (at, next) = first_call(&body)?;
        body[at.saturating_sub(4)..at]
            .iter()
            .any(|i| i.mnemonic() == Mnemonic::Movss && i.op0_register() == Register::XMM2)
            .then_some(next)
    });
    if let Some(rva) = next {
        map.insert("layout_next", rva);
    }
}

fn resolve_section(image: &Image, map: &mut Map) {
    let mut calls = Vec::new();
    for module in map.modules.values() {
        let Some(entry) = module.gui_callback else {
            continue;
        };
        let body = scan::function_body(image, entry);
        for (at, call) in body.iter().enumerate() {
            if call.flow_control() != FlowControl::Call {
                continue;
            }
            let text = body[at.saturating_sub(4)..at].iter().rev().find_map(|i| {
                (i.mnemonic() == Mnemonic::Lea
                    && i.op0_register() == Register::R8
                    && i.is_ip_rel_memory_operand())
                .then(|| image.cstr(i.ip_rel_memory_address() as usize))
                .flatten()
            });
            if let Some(text) = text {
                let target = call.near_branch_target() as usize;
                calls.push((module.name.clone(), text.to_string(), call.ip() as usize, target));
            }
        }
    }
    let candidates: BTreeSet<_> =
        calls.iter().filter(|c| c.1 == "Main").map(|c| c.3).collect();
    if candidates.len() != 1 {
        return;
    }
    let section = *candidates.first().unwrap();
    map.insert("gui_section", section);
    for (module, text, call, _) in calls.into_iter().filter(|c| c.3 == section) {
        map.sections.entry(module).or_default().push((text, call));
    }
}
