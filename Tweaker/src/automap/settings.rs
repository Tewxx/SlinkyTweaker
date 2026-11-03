use super::{image::Image, scan, Map, Setting};
use iced_x86::{FlowControl, MemorySize, Mnemonic, OpKind, Register};
use std::collections::{BTreeMap, BTreeSet};

pub use super::sync_calls::{calls, Call};

pub fn value_offset(kind: &str) -> Option<usize> {
    match kind {
        "bool" => Some(0x18),
        "float" | "enum" | "string_list" => Some(0x30),
        "range" => Some(0x40),
        "color" => Some(8),
        _ => None,
    }
}
fn descriptor_valid(image: &Image, descriptor: usize, kind: &str) -> bool {
    let size = match kind {
        "bool" => 0x20,
        "float" | "enum" => 0x38,
        "range" => 0x48,
        "color" => 0x20,
        "string_list" => 0x48,
        _ => return false,
    };
    if !image.data(descriptor, size, true) {
        return false;
    }
    match kind {
        "bool" => image.read(descriptor + 0x18, 1).is_some_and(|b| b[0] <= 1),
        "color" => (0..3).all(|i| {
            image.u32(descriptor + 8 + i * 4).is_some_and(|bits| {
                let f = f32::from_bits(bits);
                f.is_finite() && (0.0..=1.0).contains(&f)
            })
        }),
        "float" | "range" => image
            .u32(descriptor + value_offset(kind).unwrap())
            .is_some_and(|b| f32::from_bits(b).is_finite()),
        "enum" => {
            image
                .u32(descriptor + 0x20)
                .is_some_and(|n| (1..=256).contains(&n))
                && image
                    .u32(descriptor + 0x30)
                    .zip(image.u32(descriptor + 0x20))
                    .is_some_and(|(v, n)| v < n)
        }
        _ => true,
    }
}

fn linear(image: &Image, entry: usize, end: usize) -> Vec<iced_x86::Instruction> {
    let mut out = Vec::new();
    let mut at = entry;
    while let Some(i) = scan::instruction(image, at).filter(|_| at < end && out.len() < 400) {
        if i.mnemonic() == Mnemonic::Int3 {
            break;
        }
        at = i.next_ip() as usize;
        out.push(i);
    }
    out
}

fn helper_kind(image: &Image, helper: usize, end: usize) -> Option<&'static str> {
    let mut kinds = BTreeSet::new();
    for i in linear(image, helper, end) {
        if matches!(i.flow_control(), FlowControl::IndirectBranch | FlowControl::IndirectCall)
            && i.op0_kind() == OpKind::Memory
            && i.memory_displacement64() == 0x50
        {
            kinds.insert("string_list");
        }
        if !(0..i.op_count()).any(|n| i.op_kind(n) == OpKind::Memory)
            || matches!(i.memory_base(), Register::RSP | Register::RBP | Register::RIP | Register::None)
            || i.memory_index() != Register::None
        {
            continue;
        }
        let offset = i.memory_displacement64();
        let float = i.mnemonic() == Mnemonic::Movss;
        match (offset, i.memory_size()) {
            (0x18 | 0x19, MemorySize::UInt8) => kinds.insert("bool"),
            (0x38 | 0x3c, _) if float => kinds.insert("range"),
            (0x28 | 0x2c | 0x34, _) if float => kinds.insert("float"),
            (0x34, MemorySize::UInt32) if !float => kinds.insert("enum"),
            (0x14 | 0x1c, MemorySize::UInt32 | MemorySize::UInt64) => kinds.insert("color"),
            _ => false,
        };
    }
    (kinds.len() == 1).then(|| kinds.pop_first().unwrap())
}

pub fn resolve(image: &Image, map: &mut Map) {
    let mut found: BTreeMap<String, BTreeMap<String, BTreeSet<Call>>> = BTreeMap::new();
    for module in map.modules.values() {
        let Some(callback) = module.sync_callback else {
            continue;
        };
        let records = found.entry(module.name.clone()).or_default();
        for record in calls(image, callback) {
            let key = record.key.text(image).unwrap();
            if key.len() <= 64
                && key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            {
                records.entry(key).or_default().insert(record);
            }
        }
    }
    let starts: BTreeSet<usize> = found.values().flat_map(|r| r.values().flatten()).map(|c| c.helper).collect();
    let kinds_by_helper: BTreeMap<usize, Option<&'static str>> = starts
        .iter()
        .map(|&helper| {
            let end = starts.range(helper + 1..).next().copied().unwrap_or_else(|| {
                scan::function_body(image, helper).iter().map(|i| i.next_ip() as usize).max().unwrap_or(helper)
            });
            (helper, helper_kind(image, helper, end))
        })
        .collect();
    let mut helpers: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    for (name, records) in found {
        let Some(module) = map.modules.get_mut(&name) else {
            continue;
        };
        for (key, candidates) in records {
            if candidates.len() != 1 {
                continue;
            }
            let record = *candidates.first().unwrap();
            let kind = kinds_by_helper[&record.helper].unwrap_or("unknown");
            if kind != "unknown" && !descriptor_valid(image, record.descriptor, kind) {
                continue;
            }
            if kind != "unknown" {
                helpers
                    .entry(kind.into())
                    .or_default()
                    .insert(record.helper);
            }
            module.settings.insert(
                key.clone(),
                Setting {
                    key,
                    kind: kind.into(),
                    descriptor: record.descriptor,
                    value_offset: value_offset(kind),
                    gui_helper: None,
                },
            );
        }
    }
    for (kind, candidates) in helpers {
        if candidates.len() == 1 {
            map.insert(&format!("sync_{kind}"), *candidates.first().unwrap());
        } else {
            for module in map.modules.values_mut() {
                module.settings.retain(|_, s| s.kind != kind);
            }
        }
    }
    map.alias_setting("block_in_speed", "Block In", "speed");
    map.alias_setting("block_in_require_ground", "Block In", "require_ground");
    map.alias_setting("aim_assist_hor_speed", "Aim Assist", "hor_speed");
    map.alias_setting("aim_assist_ver_speed", "Aim Assist", "ver_speed");
}

