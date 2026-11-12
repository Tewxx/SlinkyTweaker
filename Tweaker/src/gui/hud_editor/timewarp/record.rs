use crate::automap;
use crate::slinky::mem::read_cstr;
use crate::slinky::sections::callback_body;
use crate::slinky::thunk::{self, Arg, FlushInstructionCache, GetCurrentProcess};
use iced_x86::{
    BlockEncoder, BlockEncoderOptions, FlowControl, InstructionBlock, InstructionInfoFactory, OpAccess, OpKind,
    Register,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Widget {
    Bool,
    Float,
    Range,
    Enum,
    Color,
    List,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item {
    Section(String),
    Setting(usize, Widget),
}

pub(crate) struct Extras {
    pub before: Option<&'static str>,
    pub items: Vec<Item>,
}

type ExtrasFn = fn() -> Extras;

static WRAPPED: Mutex<Vec<(usize, Option<usize>, ExtrasFn)>> = Mutex::new(Vec::new());

pub(crate) fn register(descriptor: usize, original: Option<usize>, extras: ExtrasFn) {
    let mut wrapped = WRAPPED.lock().unwrap_or_else(|e| e.into_inner());
    wrapped.retain(|w| w.0 != descriptor);
    wrapped.push((descriptor, original, extras));
}

type Callback = unsafe extern "system" fn(*mut c_void, *mut c_void);

#[derive(Clone, Copy)]
struct Call {
    target: usize,
    args: [usize; 3],
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

static COPIES: Mutex<Vec<(usize, Option<usize>)>> = Mutex::new(Vec::new());
static THUNKS: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
static SCRATCH: [[u64; 512]; 2] = [[0; 512]; 2];

struct Known {
    descriptors: HashMap<usize, Widget>,
    helpers: HashMap<usize, Widget>,
    section: Option<usize>,
    image: std::ops::Range<usize>,
}

fn widget(kind: &str) -> Option<Widget> {
    Some(match kind {
        "bool" => Widget::Bool,
        "float" => Widget::Float,
        "range" => Widget::Range,
        "enum" => Widget::Enum,
        "color" => Widget::Color,
        "string_list" => Widget::List,
        _ => return None,
    })
}

fn known() -> Option<&'static Known> {
    static KNOWN: OnceLock<Option<Known>> = OnceLock::new();
    KNOWN
        .get_or_init(|| {
            let map = automap::get().ok()?;
            let descriptors = map
                .modules
                .values()
                .flat_map(|m| m.settings.values())
                .filter_map(|s| Some((s.descriptor_address(map), widget(&s.kind)?)))
                .collect();
            let helpers = ["bool", "float", "range", "enum", "color", "string_list"]
                .into_iter()
                .filter_map(|k| Some((map.address(&format!("gui_{k}")).ok()?, widget(k)?)))
                .collect();
            Some(Known {
                descriptors,
                helpers,
                section: map.address("gui_section").ok(),
                image: map.base..map.base + map.image_size,
            })
        })
        .as_ref()
}

unsafe extern "system" fn recorder(rcx: usize, rdx: usize, r8: usize, target: usize) -> usize {
    CALLS.with(|calls| {
        if let Ok(mut calls) = calls.try_borrow_mut() {
            if calls.len() < 512 {
                calls.push(Call { target, args: [rcx, rdx, r8] });
            }
        }
    });
    0
}

fn thunk_for(near: usize, target: usize) -> Option<usize> {
    let mut thunks = THUNKS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&(_, stub)) = thunks.iter().find(|(t, s)| *t == target && s.abs_diff(near) < 0x4000_0000) {
        return Some(stub);
    }
    let stub = thunk::bind(near, Arg::Fourth, target, recorder as *const () as usize).ok()?;
    thunks.push((target, stub));
    Some(stub)
}

fn safe(body: &[iced_x86::Instruction]) -> bool {
    let mut factory = InstructionInfoFactory::new();
    body.iter().all(|i| {
        if matches!(i.flow_control(), FlowControl::IndirectBranch | FlowControl::IndirectCall | FlowControl::Interrupt | FlowControl::XbeginXabortXend) {
            return false;
        }
        factory.info(i).used_memory().iter().all(|m| {
            let written = matches!(
                m.access(),
                OpAccess::Write | OpAccess::ReadWrite | OpAccess::CondWrite | OpAccess::ReadCondWrite
            );
            match m.base() {
                Register::RIP => !written,
                Register::RSP | Register::RBP => m.index() == Register::None,
                _ => false,
            }
        })
    })
}

fn build(callback: usize) -> Option<usize> {
    let mut body = callback_body(callback).ok()?;
    if body.is_empty() || !safe(&body) {
        return None;
    }
    let start = body.first()?.ip();
    let end = body.last()?.next_ip();
    for inst in body.iter_mut() {
        let redirect = match inst.flow_control() {
            FlowControl::Call => true,
            FlowControl::UnconditionalBranch => !(start..end).contains(&inst.near_branch_target()),
            _ => false,
        };
        if redirect {
            if inst.op0_kind() != OpKind::NearBranch64 {
                return None;
            }
            let stub = thunk_for(callback, inst.near_branch_target() as usize)?;
            inst.set_near_branch64(stub as u64);
        }
    }
    let size = (end - start) as usize * 2 + 0x100;
    let copy = thunk::alloc_near_size(callback, size).ok()?;
    let encoded = BlockEncoder::encode(64, InstructionBlock::new(&body, copy as u64), BlockEncoderOptions::NONE).ok()?;
    let code = encoded.code_buffer;
    if code.len() > size.next_multiple_of(thunk::PAGE) {
        return None;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), copy as *mut u8, code.len());
        FlushInstructionCache(GetCurrentProcess(), copy as *const c_void, code.len());
    }
    Some(copy)
}

fn copy_of(callback: usize) -> Option<usize> {
    let mut copies = COPIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(&(_, copy)) = copies.iter().find(|(c, _)| *c == callback) {
        return copy;
    }
    let copy = build(callback);
    copies.push((callback, copy));
    copy
}

fn interpret(known: &Known, calls: &[Call]) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for call in calls {
        let item = if let Some(&w) = known.descriptors.get(&call.args[2]) {
            Some(Item::Setting(call.args[2], w))
        } else if known.section == Some(call.target) || (!known.helpers.contains_key(&call.target) && known.image.contains(&call.args[2])) {
            read_cstr(call.args[2], 96)
                .filter(|t| known.image.contains(&call.args[2]) && t.chars().all(|c| !c.is_control()))
                .map(Item::Section)
        } else if let Some(found) = call.args.iter().find_map(|&a| {
            known.descriptors.get(&a).map(|&w| (a, w)).or_else(|| {
                [(8, Widget::Color), (0x20, Widget::List)]
                    .into_iter()
                    .find_map(|(field, w)| (known.descriptors.get(&a.wrapping_sub(field)) == Some(&w)).then(|| (a - field, w)))
            })
        }) {
            Some(Item::Setting(found.0, found.1))
        } else {
            known
                .helpers
                .get(&call.target)
                .filter(|_| call.args[2] != 0)
                .map(|&w| Item::Setting(call.args[2], w))
        };
        if let Some(item) = item {
            if items.last() != Some(&item) {
                items.push(item);
            }
        }
    }
    while matches!(items.last(), Some(Item::Section(_))) {
        items.pop();
    }
    items
}

fn fallback(descriptor: usize) -> Vec<Item> {
    let Ok(map) = automap::get() else {
        return Vec::new();
    };
    let Some(module) = map.modules.values().find(|m| map.base + m.descriptor == descriptor) else {
        return Vec::new();
    };
    let mut settings: Vec<(usize, Widget)> = module
        .settings
        .values()
        .filter_map(|s| Some((s.descriptor_address(map), widget(&s.kind)?)))
        .collect();
    settings.sort_by_key(|s| s.0);
    settings.into_iter().map(|(d, w)| Item::Setting(d, w)).collect()
}

static GENERATION: AtomicU64 = AtomicU64::new(0);
const REFRESH: Duration = Duration::from_millis(500);

struct Cached {
    generation: u64,
    at: Instant,
    items: Vec<Item>,
}

thread_local! {
    static CACHE: RefCell<HashMap<(usize, Option<usize>), Cached>> = RefCell::new(HashMap::new());
}

pub(super) fn invalidate() {
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

pub(super) fn items(descriptor: usize, callback: Option<usize>) -> Vec<Item> {
    let generation = GENERATION.load(Ordering::Acquire);
    let key = (descriptor, callback);
    let cached = CACHE.with(|cache| {
        cache
            .borrow()
            .get(&key)
            .filter(|c| c.generation == generation && c.at.elapsed() < REFRESH)
            .map(|c| c.items.clone())
    });
    if let Some(items) = cached {
        return items;
    }
    let items = record_items(descriptor, callback);
    CACHE.with(|cache| cache.borrow_mut().insert(key, Cached { generation, at: Instant::now(), items: items.clone() }));
    items
}

fn record_items(descriptor: usize, callback: Option<usize>) -> Vec<Item> {
    let wrapped = WRAPPED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|w| w.0 == descriptor)
        .map(|w| (w.1, w.2));
    let callback = match wrapped {
        Some((original, _)) => original,
        None => callback,
    };
    let recorded = callback.and_then(|callback| {
        let known = known()?;
        if !known.image.contains(&callback) {
            return None;
        }
        let copy = copy_of(callback)?;
        CALLS.with(|c| c.borrow_mut().clear());
        let run = unsafe { std::mem::transmute::<usize, Callback>(copy) };
        unsafe {
            run(SCRATCH[0].as_ptr() as *mut c_void, SCRATCH[1].as_ptr() as *mut c_void);
        }
        let calls = CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()));
        Some(interpret(known, &calls))
    });
    let mut items = match (recorded, callback) {
        (Some(items), _) => items,
        (None, Some(_)) => fallback(descriptor),
        (None, None) => Vec::new(),
    };
    if let Some((_, extras)) = wrapped {
        let Extras { before, items: extra } = extras();
        let at = before
            .and_then(|text| items.iter().position(|i| matches!(i, Item::Section(t) if t == text)))
            .unwrap_or(items.len());
        items.splice(at..at, extra);
    }
    items.retain(|i| !matches!(i, Item::Setting(_, w) if !super::data::supported(*w)));
    items.retain(|i| !super::hidden(descriptor, i));
    let mut kept: Vec<Item> = Vec::new();
    for item in items {
        if matches!(item, Item::Section(_)) && matches!(kept.last(), Some(Item::Section(_))) {
            kept.pop();
        }
        kept.push(item);
    }
    while matches!(kept.last(), Some(Item::Section(_))) {
        kept.pop();
    }
    kept
}
