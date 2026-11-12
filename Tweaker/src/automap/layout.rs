use super::{image::Image, profiles, scan, Map};
use iced_x86::{FlowControl, Instruction, Mnemonic, OpKind, Register};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleLayout {
    pub name: usize,
    pub description: usize,
    pub enabled: usize,
    pub flags: usize,
    pub keybind: usize,
    pub hold: usize,
    pub gui: usize,
    pub sync: usize,
    pub no_toggle: u32,
    pub no_keybind: u32,
    pub no_hold: u32,
}

impl ModuleLayout {
    pub const KNOWN: Self = Self {
        name: 0,
        description: 8,
        enabled: 0x14,
        flags: 0x18,
        keybind: 0x1c,
        hold: 0x20,
        gui: 0x1f8,
        sync: 0x200,
        no_toggle: 2,
        no_keybind: 1,
        no_hold: 8,
    };
    pub fn span(&self) -> usize {
        self.gui.max(self.sync) + 0x10
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoolLayout {
    pub label: usize,
    pub value: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FloatLayout {
    pub label: usize,
    pub format: usize,
    pub min: usize,
    pub max: usize,
    pub step: usize,
    pub mult: usize,
    pub value: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RangeLayout {
    pub label: usize,
    pub format: usize,
    pub min: usize,
    pub max: usize,
    pub step: usize,
    pub mult: usize,
    pub low: usize,
    pub high: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnumLayout {
    pub label: usize,
    pub count: usize,
    pub table: usize,
    pub value: usize,
    pub nullable: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorLayout {
    pub label: usize,
    pub rgb: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListLayout {
    pub label: usize,
    pub edit: Option<ListEdit>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListEdit {
    pub mode: usize,
    pub storage: usize,
    pub count: usize,
    pub get: usize,
    pub color: usize,
    pub add: usize,
    pub remove: usize,
    pub options: usize,
    pub dirty: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Layout {
    pub module: Option<ModuleLayout>,
    pub bool: Option<BoolLayout>,
    pub float: Option<FloatLayout>,
    pub range: Option<RangeLayout>,
    pub choice: Option<EnumLayout>,
    pub color: Option<ColorLayout>,
    pub list: Option<ListLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Value {
    Descriptor,
    Load(usize, usize),
    Float(usize),
    Address(usize),
    Other,
}

fn field(i: &Instruction) -> Option<(Register, usize, usize)> {
    if !(0..i.op_count()).any(|n| i.op_kind(n) == OpKind::Memory)
        || i.is_ip_rel_memory_operand()
        || i.memory_index() != Register::None
        || matches!(i.memory_base(), Register::None | Register::RIP)
    {
        return None;
    }
    let disp = i.memory_displacement64() as usize;
    (disp < 0x1000).then(|| (i.memory_base(), disp, i.memory_size().size()))
}

fn linear(image: &Image, entry: usize, limit: usize) -> Vec<Instruction> {
    let mut out = Vec::new();
    let mut at = entry;
    while let Some(i) = scan::instruction(image, at).filter(|_| out.len() < limit) {
        at = i.next_ip() as usize;
        let stop = matches!(i.mnemonic(), Mnemonic::Ret | Mnemonic::Int3);
        out.push(i);
        if stop {
            break;
        }
    }
    out
}

fn string_sites(image: &Image, text: &str) -> BTreeSet<usize> {
    image
        .string_hits(text)
        .into_iter()
        .flat_map(|hit| profiles::string_refs(image, hit))
        .collect()
}

fn lea_index(body: &[Instruction], sites: &BTreeSet<usize>) -> Option<usize> {
    body.iter().position(|i| sites.contains(&(i.ip() as usize)))
}

struct Routine {
    body: Vec<Instruction>,
    on: usize,
    bind: usize,
    hold: usize,
    descriptor: Register,
    flags: usize,
}

fn routine(image: &Image, entry: usize, sites: [&BTreeSet<usize>; 3]) -> Option<Routine> {
    let body = linear(image, entry, 256);
    let on = lea_index(&body, sites[0])?;
    let bind = lea_index(&body, sites[1])?;
    let hold = lea_index(&body, sites[2])?;
    if !(on < bind && bind < hold) {
        return None;
    }
    let (descriptor, flags) = body[..on].windows(2).find_map(|w| {
        let (base, disp, size) = field(&w[0])?;
        (w[0].mnemonic() == Mnemonic::Mov
            && size == 4
            && w[0].op0_kind() == OpKind::Register
            && w[1].mnemonic() == Mnemonic::Test
            && w[1].op0_kind() == OpKind::Register
            && w[1].op0_register().full_register() == w[0].op0_register().full_register()
            && w[1].op1_kind() == OpKind::Immediate8)
            .then_some((base, disp))
    })?;
    Some(Routine { body, on, bind, hold, descriptor, flags })
}

impl Routine {
    fn on_descriptor(&self, i: &Instruction) -> Option<(usize, usize)> {
        field(i)
            .filter(|f| f.0 == self.descriptor)
            .map(|f| (f.1, f.2))
    }
    fn flag_tests(&self, range: std::ops::Range<usize>) -> Option<u32> {
        self.body[range].iter().rev().find_map(|i| {
            let tests_flags = i.mnemonic() == Mnemonic::Test
                && i.op1_kind() == OpKind::Immediate8
                && (i.op0_kind() == OpKind::Register
                    || self.on_descriptor(i).is_some_and(|f| f.0 == self.flags));
            tests_flags.then(|| i.immediate8() as u32)
        })
    }
    fn byte_compare(&self, range: std::ops::Range<usize>) -> Option<usize> {
        self.body[range].iter().rev().find_map(|i| {
            (i.mnemonic() == Mnemonic::Cmp && i.op1_kind() == OpKind::Immediate8 && i.immediate8() == 1)
                .then(|| self.on_descriptor(i))
                .flatten()
                .filter(|f| f.1 == 1)
                .map(|f| f.0)
        })
    }
    fn callback_after(&self, start: usize) -> Option<usize> {
        let body = &self.body[start..];
        body.iter().enumerate().find_map(|(n, i)| {
            let (disp, size) = self.on_descriptor(i)?;
            if i.mnemonic() != Mnemonic::Mov || size != 8 || i.op0_kind() != OpKind::Register {
                return None;
            }
            let target = i.op0_register();
            body[n + 1..(n + 10).min(body.len())]
                .iter()
                .any(|c| {
                    c.flow_control() == FlowControl::IndirectCall
                        && c.op0_kind() == OpKind::Register
                        && c.op0_register() == target
                })
                .then_some(disp)
        })
    }
    fn descriptor_call(&self, range: std::ops::Range<usize>) -> Option<usize> {
        let start = range.start;
        self.body[range].iter().enumerate().rev().find_map(|(n, i)| {
            if i.flow_control() != FlowControl::Call || i.op0_kind() != OpKind::NearBranch64 {
                return None;
            }
            let at = start + n;
            self.body[at.saturating_sub(3).max(start)..at]
                .iter()
                .any(|m| {
                    m.mnemonic() == Mnemonic::Mov
                        && m.op0_kind() == OpKind::Register
                        && m.op0_register() == Register::RDX
                        && m.op1_kind() == OpKind::Register
                        && m.op1_register() == self.descriptor
                })
                .then(|| i.near_branch_target() as usize)
        })
    }
}

struct Saved {
    name: usize,
    enabled: usize,
    flags: usize,
    keybind: usize,
    hold: usize,
    sync: usize,
    bits: [u32; 3],
}

fn save_shape(r: &Routine) -> Option<Saved> {
    let enabled = r.byte_compare(0..r.on)?;
    let hold = r.byte_compare(r.bind..r.hold)?;
    let keybind = r.body[r.on..r.bind].iter().rev().find_map(|i| {
        let (disp, size) = r.on_descriptor(i)?;
        (i.mnemonic() == Mnemonic::Mov && size == 4 && disp != r.flags).then_some(disp)
    })?;
    let mut aliases = BTreeSet::from([r.descriptor]);
    for i in &r.body[..r.on] {
        if i.mnemonic() == Mnemonic::Mov
            && i.op0_kind() == OpKind::Register
            && i.op1_kind() == OpKind::Register
            && i.op0_register() == r.descriptor
        {
            aliases.insert(i.op1_register());
        }
    }
    let name = r.body[..r.on].iter().find_map(|i| {
        let (base, disp, size) = field(i)?;
        (i.mnemonic() == Mnemonic::Mov && size == 8 && aliases.contains(&base)).then_some(disp)
    })?;
    let bits = [
        r.flag_tests(0..r.on)?,
        r.flag_tests(r.on..r.bind)?,
        r.flag_tests(r.bind..r.hold)?,
    ];
    let sync = r.callback_after(r.hold)?;
    Some(Saved { name, enabled, flags: r.flags, keybind, hold, sync, bits })
}

fn loaded_hold(r: &Routine) -> Option<usize> {
    r.body[r.hold..].iter().find_map(|i| {
        let (disp, size) = r.on_descriptor(i)?;
        (i.mnemonic() == Mnemonic::Mov && size == 1 && i.op0_kind() == OpKind::Memory).then_some(disp)
    })
}

pub fn resolve_module(image: &Image, map: &mut Map) -> Option<ModuleLayout> {
    let sites = [
        string_sites(image, "__on"),
        string_sites(image, "__bind"),
        string_sites(image, "__hold"),
    ];
    let entries: BTreeSet<usize> = sites[0]
        .iter()
        .filter_map(|&at| profiles::function_start(image, at))
        .collect();
    let routines: Vec<Routine> = entries
        .iter()
        .filter_map(|&entry| routine(image, entry, [&sites[0], &sites[1], &sites[2]]))
        .collect();
    let saves: Vec<Saved> = routines.iter().filter_map(save_shape).collect();
    let [save] = &saves[..] else {
        return None;
    };
    let mut bits = save.bits;
    bits.sort_unstable();
    let offsets = [save.name, save.enabled, save.flags, save.keybind, save.hold, save.sync];
    if bits.iter().any(|b| !b.is_power_of_two())
        || bits.windows(2).any(|w| w[0] == w[1])
        || offsets.iter().collect::<BTreeSet<_>>().len() != offsets.len()
    {
        return None;
    }
    let loads: Vec<&Routine> = routines
        .iter()
        .filter(|r| save_shape(r).is_none())
        .filter(|r| r.flags == save.flags && loaded_hold(r) == Some(save.hold) && r.callback_after(r.hold) == Some(save.sync))
        .collect();
    if let [load] = loads[..] {
        if let Some(set_enabled) = load.descriptor_call(load.on..load.bind) {
            map.insert("module_set_enabled", set_enabled);
        }
        if let Some(set_keybind) = load.descriptor_call(load.bind..load.hold) {
            map.insert("module_set_keybind", set_keybind);
        }
    }
    let gui = gui_offset(image, save.sync)?;
    let description = description_offset(image, save.name, [gui, save.sync])?;
    let layout = ModuleLayout {
        name: save.name,
        description,
        enabled: save.enabled,
        flags: save.flags,
        keybind: save.keybind,
        hold: save.hold,
        gui,
        sync: save.sync,
        no_toggle: save.bits[0],
        no_keybind: save.bits[1],
        no_hold: save.bits[2],
    };
    (!offsets.contains(&gui) && !offsets.contains(&description)).then_some(layout)
}

fn gui_offset(image: &Image, sync: usize) -> Option<usize> {
    let mut found = BTreeSet::new();
    for section in image.sections.iter().filter(|s| s.executable) {
        let bytes = &image.bytes[section.range.clone()];
        for (n, w) in bytes.windows(3).enumerate() {
            if !((0x48..=0x4f).contains(&w[0]) && w[1] == 0x8b && w[2] >> 6 == 2) {
                continue;
            }
            let at = section.range.start + n;
            let Some(load) = scan::instruction(image, at) else {
                continue;
            };
            let Some((base, disp, 8)) = field(&load) else {
                continue;
            };
            if disp == sync
                || load.op0_kind() != OpKind::Register
                || matches!(base, Register::RSP | Register::RBP)
            {
                continue;
            }
            let target = load.op0_register();
            let body = linear(image, load.next_ip() as usize, 6);
            let shaped = body.len() >= 3
                && body[0].mnemonic() == Mnemonic::Test
                && body[0].op0_kind() == OpKind::Register
                && body[0].op0_register() == target
                && body[0].op1_kind() == OpKind::Register
                && body[0].op1_register() == target
                && body[1].flow_control() == FlowControl::ConditionalBranch
                && body[2..].iter().any(|c| {
                    c.flow_control() == FlowControl::IndirectCall
                        && c.op0_kind() == OpKind::Register
                        && c.op0_register() == target
                });
            if shaped && checked_by_card(image, at, base, disp) {
                found.insert(disp);
            }
        }
    }
    (found.len() == 1).then(|| *found.first().unwrap())
}

fn checked_by_card(image: &Image, at: usize, base: Register, disp: usize) -> bool {
    (1..=0x60).any(|back| {
        let Some(start) = at.checked_sub(back) else {
            return false;
        };
        let body = linear(image, start, 32);
        let Some(end) = body.iter().position(|i| i.ip() as usize == at) else {
            return false;
        };
        body[..end].iter().enumerate().any(|(n, call)| {
            call.flow_control() == FlowControl::Call
                && call.op0_kind() == OpKind::NearBranch64
                && body[n.saturating_sub(4)..n].iter().any(|m| {
                    m.mnemonic() == Mnemonic::Mov
                        && m.op0_kind() == OpKind::Register
                        && m.op0_register() == Register::R8
                        && m.op1_kind() == OpKind::Register
                        && m.op1_register() == base
                })
                && linear(image, call.near_branch_target() as usize, 32).iter().any(|c| {
                    c.mnemonic() == Mnemonic::Cmp
                        && field(c).is_some_and(|f| f == (Register::R8, disp, 8))
                        && c.op1_kind() != OpKind::Register
                })
        })
    })
}

fn description_offset(image: &Image, name: usize, callbacks: [usize; 2]) -> Option<usize> {
    let span = callbacks[0].max(callbacks[1]) + 8;
    let mut votes: BTreeMap<usize, usize> = BTreeMap::new();
    let text = |at: usize| image.u64(at).and_then(|p| image.rva(p as usize)).and_then(|r| image.cstr(r));
    for section in image.sections.iter().filter(|s| !s.executable && s.writable) {
        for at in (section.range.start..section.range.end.saturating_sub(span)).step_by(8) {
            let Some(title) = text(at + name) else {
                continue;
            };
            if title.len() > 48 || title.ends_with('.') || !image.data(at, span, true) {
                continue;
            }
            let pointers = callbacks.map(|o| image.u64(at + o).unwrap_or(u64::MAX) as usize);
            let callable = pointers
                .iter()
                .all(|&p| p == 0 || image.rva(p).is_some_and(|r| image.executable(r)));
            if !callable || pointers == [0, 0] {
                continue;
            }
            for offset in (0..0x40).step_by(8).filter(|&o| o != name) {
                if text(at + offset).is_some_and(|d| d.ends_with('.') && d.len() >= 12 && d.contains(' ')) {
                    *votes.entry(offset).or_default() += 1;
                }
            }
        }
    }
    let mut ranked: Vec<_> = votes.into_iter().collect();
    ranked.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    match ranked[..] {
        [(offset, best), ..] if best >= 10 && ranked.get(1).is_none_or(|&(_, next)| next * 4 < best) => Some(offset),
        _ => None,
    }
}

const VOLATILE: [Register; 13] = [
    Register::RAX,
    Register::RCX,
    Register::RDX,
    Register::R8,
    Register::R9,
    Register::R10,
    Register::R11,
    Register::XMM0,
    Register::XMM1,
    Register::XMM2,
    Register::XMM3,
    Register::XMM4,
    Register::XMM5,
];

fn widget_args(image: &Image, helper: usize) -> Option<Vec<Value>> {
    let mut registers: BTreeMap<Register, Value> = BTreeMap::new();
    registers.insert(Register::R8, Value::Descriptor);
    let mut stack: BTreeMap<usize, Value> = BTreeMap::new();
    let read = |registers: &BTreeMap<Register, Value>, r: Register| {
        registers.get(&r.full_register()).copied().unwrap_or(Value::Other)
    };
    for i in linear(image, helper, 64) {
        match i.flow_control() {
            FlowControl::Call if i.op0_kind() == OpKind::NearBranch64 => {
                let mut args: Vec<Value> = [Register::RCX, Register::RDX, Register::R8, Register::R9]
                    .iter()
                    .map(|&r| read(&registers, r))
                    .collect();
                args.extend(stack.range(0x20..).map(|(_, v)| *v));
                return Some(args);
            }
            FlowControl::IndirectCall => {
                for r in VOLATILE {
                    registers.insert(r.full_register(), Value::Other);
                }
                continue;
            }
            FlowControl::Return | FlowControl::IndirectBranch | FlowControl::Exception => return None,
            _ => (),
        }
        if i.op_count() == 0 {
            continue;
        }
        if i.op0_kind() == OpKind::Memory {
            if i.memory_base() == Register::RSP && i.memory_index() == Register::None {
                let slot = i.memory_displacement64() as usize;
                let value = if i.op_count() > 1 && i.op1_kind() == OpKind::Register {
                    read(&registers, i.op1_register())
                } else {
                    Value::Other
                };
                stack.insert(slot, value);
            }
            continue;
        }
        if i.op0_kind() != OpKind::Register {
            continue;
        }
        let dest = i.op0_register().full_register();
        let value = match (i.mnemonic(), i.op_count() > 1 && i.op1_kind() == OpKind::Memory) {
            (Mnemonic::Lea | Mnemonic::Mov | Mnemonic::Movzx | Mnemonic::Movss, true) => {
                let base = i.memory_base();
                match field(&i) {
                    Some((_, disp, size)) if read(&registers, base) == Value::Descriptor => match i.mnemonic() {
                        Mnemonic::Lea => Value::Address(disp),
                        Mnemonic::Movss => Value::Float(disp),
                        _ => Value::Load(disp, size),
                    },
                    _ => Value::Other,
                }
            }
            (Mnemonic::Mov, false) if i.op1_kind() == OpKind::Register => read(&registers, i.op1_register()),
            _ => Value::Other,
        };
        registers.insert(dest, value);
    }
    None
}

struct Args {
    qwords: Vec<usize>,
    dwords: Vec<usize>,
    bytes: Vec<usize>,
    floats: Vec<usize>,
    addresses: Vec<usize>,
}

fn sort_args(args: &[Value]) -> Args {
    let loads = |width: usize| {
        args.iter()
            .filter_map(|v| match *v {
                Value::Load(d, size) if size == width => Some(d),
                _ => None,
            })
            .collect()
    };
    Args {
        qwords: loads(8),
        dwords: loads(4),
        bytes: loads(1),
        floats: args.iter().filter_map(|v| if let Value::Float(d) = *v { Some(d) } else { None }).collect(),
        addresses: args.iter().filter_map(|v| if let Value::Address(d) = *v { Some(d) } else { None }).collect(),
    }
}

fn helper_args(image: &Image, map: &Map, kind: &str) -> Option<Args> {
    let helper = map.anchors.get(&format!("gui_{kind}"))?.rva;
    widget_args(image, helper).map(|a| sort_args(&a))
}

fn descriptors<'a>(map: &'a Map, kind: &'a str) -> impl Iterator<Item = usize> + 'a {
    map.modules
        .values()
        .flat_map(|m| m.settings.values())
        .filter(move |s| s.kind == kind)
        .map(|s| s.descriptor)
}

fn validated<T>(image: &Image, map: &Map, kind: &str, layout: T, valid: impl Fn(&Image, usize, &T) -> bool) -> Option<T> {
    let all: Vec<usize> = descriptors(map, kind).collect();
    (!all.is_empty() && all.iter().all(|&d| valid(image, d, &layout))).then_some(layout)
}

fn label_ok(image: &Image, at: usize) -> bool {
    image
        .u64(at)
        .and_then(|p| image.rva(p as usize))
        .and_then(|r| image.cstr(r))
        .is_some()
}

fn finite(image: &Image, at: usize) -> Option<f32> {
    image.u32(at).map(f32::from_bits).filter(|v| v.is_finite())
}

pub fn resolve_settings(image: &Image, map: &mut Map) {
    let mut layout = map.layout;
    layout.bool = helper_args(image, map, "bool")
        .and_then(|a| match (&a.qwords[..], &a.addresses[..], a.floats.is_empty()) {
            ([label], [value], true) => Some(BoolLayout { label: *label, value: *value }),
            _ => None,
        })
        .and_then(|l| {
            validated(image, map, "bool", l, |image, d, l| {
                label_ok(image, d + l.label) && image.read(d + l.value, 1).is_some_and(|b| b[0] <= 1)
            })
        });
    layout.float = helper_args(image, map, "float")
        .and_then(|a| match (&a.qwords[..], &a.floats[..], &a.addresses[..]) {
            ([label, format], [min, max, step, mult], [value]) => Some(FloatLayout {
                label: *label,
                format: *format,
                min: *min,
                max: *max,
                step: *step,
                mult: *mult,
                value: *value,
            }),
            _ => None,
        })
        .and_then(|l| {
            validated(image, map, "float", l, |image, d, l| {
                let (Some(min), Some(max), Some(_)) = (finite(image, d + l.min), finite(image, d + l.max), finite(image, d + l.value)) else {
                    return false;
                };
                label_ok(image, d + l.label) && min <= max
            })
        });
    layout.range = helper_args(image, map, "range")
        .and_then(|a| match (&a.qwords[..], &a.floats[..], &a.addresses[..]) {
            ([label, format, ..], [min, max, step, mult], [low, high]) => Some(RangeLayout {
                label: *label,
                format: *format,
                min: *min,
                max: *max,
                step: *step,
                mult: *mult,
                low: *low,
                high: *high,
            }),
            _ => None,
        })
        .and_then(|l| {
            validated(image, map, "range", l, |image, d, l| {
                let values = [l.min, l.max, l.low, l.high].map(|o| finite(image, d + o));
                label_ok(image, d + l.label)
                    && values.iter().all(Option::is_some)
                    && values[0] <= values[1]
                    && values[2] <= values[3]
            })
        });
    layout.choice = helper_args(image, map, "enum")
        .and_then(|a| match (&a.qwords[..], &a.dwords[..], &a.addresses[..], &a.bytes[..]) {
            ([label, table], [count], [value], [nullable]) => Some(EnumLayout {
                label: *label,
                count: *count,
                table: *table,
                value: *value,
                nullable: *nullable,
            }),
            _ => None,
        })
        .and_then(|l| {
            validated(image, map, "enum", l, |image, d, l| {
                let (Some(count), Some(index)) = (image.u32(d + l.count), image.u32(d + l.value)) else {
                    return false;
                };
                let table = image.u64(d + l.table).and_then(|p| image.rva(p as usize));
                label_ok(image, d + l.label)
                    && (1..=256).contains(&count)
                    && index < count
                    && table.is_some_and(|t| (0..count as usize).all(|n| label_ok(image, t + n * 8)))
            })
        });
    layout.color = helper_args(image, map, "color")
        .and_then(|a| match (&a.qwords[..], &a.addresses[..]) {
            ([label], [rgb]) => Some(ColorLayout { label: *label, rgb: *rgb }),
            _ => None,
        })
        .and_then(|l| {
            validated(image, map, "color", l, |image, d, l| {
                label_ok(image, d + l.label)
                    && (0..3).all(|n| finite(image, d + l.rgb + n * 4).is_some_and(|v| (0.0..=1.0).contains(&v)))
            })
        });
    layout.list = helper_args(image, map, "string_list")
        .and_then(|a| a.qwords.first().map(|&label| ListLayout { label, edit: None }))
        .and_then(|l| validated(image, map, "string_list", l, |image, d, l| label_ok(image, d + l.label)))
        .map(|l| ListLayout { edit: list_edit(image, map, l.label), ..l });
    map.layout = layout;
}

fn list_edit(image: &Image, map: &Map, label: usize) -> Option<ListEdit> {
    let storage = label + 0x20;
    let edit = ListEdit {
        mode: label + 0x18,
        storage,
        count: storage + 8,
        get: storage + 0x10,
        color: storage + 0x18,
        add: storage + 0x20,
        remove: storage + 0x28,
        options: storage + 0x38,
        dirty: storage + 0x40,
    };
    let code = |image: &Image, at: usize| image.u64(at).and_then(|p| image.rva(p as usize)).is_some_and(|r| image.executable(r));
    validated(image, map, "string_list", edit, |image, d, e| {
        let optional = |at: usize| image.u64(at).is_some_and(|p| p == 0 || code(image, at));
        let mode = image.u32(d + e.mode);
        [e.count, e.get, e.add, e.remove].iter().all(|&f| code(image, d + f))
            && optional(d + e.options)
            && mode.is_some_and(|m| m <= 1)
            && if mode == Some(1) { code(image, d + e.color) } else { optional(d + e.color) }
            && image.read(d + e.dirty, 1).is_some_and(|b| b[0] <= 1)
    })
}

pub fn module_valid(image: &Image, map: &Map) -> bool {
    let Some(layout) = map.layout.module else {
        return false;
    };
    let checked = map.modules.values().filter(|m| {
        let at = m.descriptor;
        image.read(at + layout.enabled, 1).is_some_and(|b| b[0] <= 1)
            && image.read(at + layout.hold, 1).is_some_and(|b| b[0] <= 1)
            && image.u32(at + layout.keybind).is_some_and(|k| k < 0x200)
            && image.u64(at + layout.description).and_then(|p| image.rva(p as usize)).and_then(|r| image.cstr(r)).is_some()
    });
    checked.count() == map.modules.len() && map.modules.len() >= 10
}
