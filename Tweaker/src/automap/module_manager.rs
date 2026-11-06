pub(super) mod enable_batch;

use super::{events::SLOT_COUNT, image::Image, scan, Map};
use crate::{Error, Result};
use iced_x86::{Mnemonic, OpKind};
use std::collections::BTreeSet;
use std::ops::Range;

const CATEGORY_COUNT: usize = 5;
const TABLE_STRIDE: usize = 0x10;
const DESCRIPTOR_HOOKS: usize = 0x48;
const SEARCH_SPAN: usize = 0x100;
const MAX_MODULES: usize = 512;
const MAX_LISTENERS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub master: usize,
    pub categories: usize,
    pub candidates: usize,
    pub active: usize,
    pub pending: [usize; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tables {
    master: usize,
    categories: usize,
    candidates: usize,
    active: usize,
}

type Read<'a> = &'a dyn Fn(usize, usize) -> Option<Vec<u8>>;

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "module_manager_ref",
        "55 56 53 48 83 EC 20 48 8D 6C 24 20 48 89 CE 80",
    );
    if let Some(host) = map.anchors.get("module_manager_ref") {
        let targets = scan::rip_leas(image, host.rva);
        if let Some(&manager) = targets.first().filter(|&&r| image.data(r, 0x308, true)) {
            map.insert("module_manager", manager);
        }
    }
}

pub fn layout() -> Result<Layout> {
    super::get()?
        .manager_layout
        .ok_or_else(|| Error::Jvm("automap: module manager layout unresolved".into()))
}

pub fn resolve_layout(image: &Image, map: &Map, read: Read) -> Result<Layout> {
    let manager = map.address("module_manager")?;
    let known: BTreeSet<_> = map
        .modules
        .values()
        .map(|m| map.base + m.descriptor)
        .collect();
    let image_range = map.base..map.base + map.image_size;
    let tables = resolve_tables(manager, &image_range, &known, read)?;
    let pending = pending_offsets(image, map)?;
    validate_pending(manager, &tables, pending, read)?;
    Ok(Layout {
        master: tables.master,
        categories: tables.categories,
        candidates: tables.candidates,
        active: tables.active,
        pending,
    })
}

fn word(read: Read, at: usize) -> Option<usize> {
    let bytes = read(at, 8)?;
    Some(u64::from_le_bytes(bytes.as_slice().try_into().ok()?) as usize)
}

fn table(read: Read, at: usize, limit: usize) -> Option<Vec<usize>> {
    let (pointer, count) = (word(read, at)?, word(read, at + 8)?);
    if count > limit {
        return None;
    }
    if count == 0 {
        return Some(Vec::new());
    }
    if pointer == 0 {
        return None;
    }
    let bytes = read(pointer, count * 8)?;
    Some(
        bytes
            .chunks_exact(8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()) as usize)
            .collect(),
    )
}

fn resolve_tables(
    manager: usize,
    image: &Range<usize>,
    known: &BTreeSet<usize>,
    read: Read,
) -> Result<Tables> {
    let (master, modules) = find_master(manager, known, read).ok_or_else(|| {
        Error::Jvm("automap: module list not found in manager (not ready, or layout changed)".into())
    })?;
    let categories = find_categories(manager, master, &modules, read).ok_or_else(|| {
        Error::Jvm("automap: category tables not found in manager".into())
    })?;
    let table_start = categories + CATEGORY_COUNT * TABLE_STRIDE;
    let (candidates, hooks) = (table_start..table_start + SEARCH_SPAN)
        .step_by(8)
        .find_map(|at| candidate_hooks(manager + at, read).map(|hooks| (at, hooks)))
        .ok_or_else(|| Error::Jvm("automap: listener tables not found in manager".into()))?;
    let active_start = candidates + SLOT_COUNT * TABLE_STRIDE;
    let active = (active_start..active_start + SEARCH_SPAN)
        .step_by(8)
        .find(|&at| active_matches(manager + at, &hooks, image, read))
        .ok_or_else(|| Error::Jvm("automap: active listener table not found in manager".into()))?;
    Ok(Tables {
        master,
        categories,
        candidates,
        active,
    })
}

fn find_master(manager: usize, known: &BTreeSet<usize>, read: Read) -> Option<(usize, Vec<usize>)> {
    (0..SEARCH_SPAN)
        .step_by(8)
        .filter_map(|at| Some((at, table(read, manager + at, MAX_MODULES)?)))
        .filter(|(_, modules)| {
            let matched = modules.iter().filter(|m| known.contains(m)).count();
            modules.len() >= 10 && matched >= 10 && matched * 2 >= modules.len()
        })
        .max_by_key(|(at, modules)| (modules.len(), std::cmp::Reverse(*at)))
}

fn find_categories(manager: usize, master: usize, modules: &[usize], read: Read) -> Option<usize> {
    let all: BTreeSet<_> = modules.iter().copied().collect();
    let start = master + TABLE_STRIDE;
    (start..start + SEARCH_SPAN).step_by(8).find(|&at| {
        let mut total = 0;
        (0..CATEGORY_COUNT).all(|category| {
            table(read, manager + at + category * TABLE_STRIDE, MAX_MODULES).is_some_and(|entries| {
                total += entries.len();
                !entries.is_empty() && entries.iter().all(|e| all.contains(e))
            })
        }) && total == modules.len()
    })
}

fn candidate_hooks(at: usize, read: Read) -> Option<Vec<Vec<usize>>> {
    let mut hooks = Vec::with_capacity(SLOT_COUNT);
    for slot in 0..SLOT_COUNT {
        let mut callbacks = Vec::new();
        for owner in table(read, at + slot * TABLE_STRIDE, MAX_LISTENERS)? {
            let callback = word(read, owner + DESCRIPTOR_HOOKS + slot * TABLE_STRIDE)?;
            if callback == 0 {
                return None;
            }
            callbacks.push(callback);
        }
        hooks.push(callbacks);
    }
    (hooks.iter().map(Vec::len).sum::<usize>() >= 10).then_some(hooks)
}

fn active_matches(at: usize, hooks: &[Vec<usize>], image: &Range<usize>, read: Read) -> bool {
    hooks.iter().enumerate().all(|(slot, callbacks)| {
        let Some(list) = word(read, at + slot * 8) else {
            return false;
        };
        if list == 0 {
            return callbacks.is_empty();
        }
        for index in 0..MAX_LISTENERS {
            match word(read, list + index * 8) {
                Some(0) => return true,
                Some(callback) if image.contains(&callback) && callbacks.contains(&callback) => {}
                _ => return false,
            }
        }
        false
    })
}

fn pending_offsets(image: &Image, map: &Map) -> Result<[usize; 2]> {
    let unresolved = || Error::Jvm("automap: pending module queues not found in code".into());
    let host = map.anchors.get("module_manager_ref").ok_or_else(unresolved)?.rva;
    let manager = map.anchors.get("module_manager").ok_or_else(unresolved)?.rva;
    let mut at = host;
    let mut loaded = false;
    let mut callee = None;
    for _ in 0..64 {
        let inst = scan::instruction(image, at).ok_or_else(unresolved)?;
        if inst.mnemonic() == Mnemonic::Lea
            && inst.is_ip_rel_memory_operand()
            && inst.ip_rel_memory_address() as usize == manager
        {
            loaded = true;
        } else if loaded && inst.mnemonic() == Mnemonic::Call && inst.op0_kind() == OpKind::NearBranch64 {
            callee = Some(inst.near_branch_target() as usize);
            break;
        }
        at = inst.next_ip() as usize;
    }
    let offsets: BTreeSet<usize> = scan::function_body(image, callee.ok_or_else(unresolved)?)
        .into_iter()
        .filter(|i| {
            i.mnemonic() == Mnemonic::Mov
                && i.op0_kind() == OpKind::Register
                && i.op1_kind() == OpKind::Immediate32
        })
        .map(|i| i.immediate32() as usize)
        .filter(|offset| (0x100..0x1000).contains(offset))
        .collect();
    match offsets.into_iter().collect::<Vec<_>>()[..] {
        [first, second] if second - first == TABLE_STRIDE => Ok([first, second]),
        _ => Err(unresolved()),
    }
}

fn validate_pending(manager: usize, tables: &Tables, pending: [usize; 2], read: Read) -> Result<()> {
    let invalid = || Error::Jvm("automap: pending module queues failed validation".into());
    let modules = word(read, manager + tables.master + 8).ok_or_else(invalid)?;
    for offset in pending {
        let after_active = offset >= tables.active + SLOT_COUNT * 8;
        let pointer = word(read, manager + offset).ok_or_else(invalid)?;
        let count = word(read, manager + offset + 8).ok_or_else(invalid)?;
        if !after_active || pointer == 0 || count > modules {
            return Err(invalid());
        }
    }
    Ok(())
}
