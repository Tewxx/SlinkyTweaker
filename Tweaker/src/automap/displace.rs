use super::{image::Image, scan, Event, Map};
use iced_x86::{FlowControl, Instruction, Mnemonic, OpKind, Register};
use std::collections::BTreeSet;

const MODULE: &str = "Knockback Displace";
pub const REQUIRED: &[&str] = &[
    "native_displace_yaw",
    "native_holding_weapon",
    "native_player_tagged",
    "native_as_other_player",
];

fn value(map: &Map, key: &str) -> Option<usize> {
    map.setting(MODULE, key)
        .and_then(|s| s.value_offset.map(|v| s.descriptor + v))
}
fn reads(i: &Instruction, address: usize) -> bool {
    i.is_ip_rel_memory_operand() && i.ip_rel_memory_address() as usize == address
}

pub fn resolve(image: &Image, map: &mut Map) {
    let Some(entry) = map.hook(MODULE, Event::ClickPre).map(|h| h.callback) else {
        return;
    };
    let (Some(direction), Some(require_weapon)) = (value(map, "direction"), value(map, "require_weapon"))
    else {
        return;
    };
    let found = candidates(image, entry, direction, require_weapon);
    for (name, found) in REQUIRED.iter().zip(found) {
        if found.len() == 1 {
            map.insert(name, *found.first().unwrap());
        }
    }
}

fn candidates(image: &Image, entry: usize, direction: usize, require_weapon: usize) -> [BTreeSet<usize>; 4] {
    let body = scan::function_body(image, entry);
    let mut found: [BTreeSet<usize>; 4] = Default::default();
    let mut previous_call = None;
    for (at, call) in body.iter().enumerate() {
        if call.flow_control() != FlowControl::Call {
            continue;
        }
        let target = call.near_branch_target() as usize;
        if target == 0 {
            continue;
        }
        let head: Vec<_> = scan::function_body(image, target).into_iter().take(40).collect();
        if head.iter().any(|i| reads(i, direction))
            && head.iter().any(|i| i.flow_control() == FlowControl::IndirectBranch)
        {
            found[0].insert(target);
        }
        let before = &body[at.saturating_sub(3)..at];
        let flags_out = before
            .iter()
            .any(|i| i.mnemonic() == Mnemonic::Lea && i.op0_register() == Register::R8);
        let checked = body[at + 1..(at + 3).min(body.len())]
            .iter()
            .any(|i| i.mnemonic() == Mnemonic::Cmp && reads(i, require_weapon));
        if flags_out && checked {
            found[1].insert(target);
        }
        let tag_mask = before.iter().any(|i| {
            i.mnemonic() == Mnemonic::Mov
                && i.op0_register() == Register::R8D
                && i.op1_kind() == OpKind::Immediate32
                && i.immediate32() == 7
        });
        if tag_mask {
            found[2].insert(target);
            found[3].extend(previous_call);
        }
        previous_call = Some(target);
    }
    found
}
