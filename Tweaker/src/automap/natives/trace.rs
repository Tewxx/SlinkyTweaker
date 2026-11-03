use crate::automap::{image::Image, scan};
use iced_x86::{FlowControl, MemorySize, Mnemonic, OpKind, Register};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Version,
    Slot(usize),
    Jni(usize),
    Call(usize),
    Tail(usize),
    Index(u32),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Value {
    Argument(Register),
    Load(usize, usize),
    Fresh(usize),
}
enum Raw {
    Load(usize),
    Compare(usize),
    CallMemory(usize, usize),
    Register(usize),
    Step(Step),
}

#[derive(Default)]
struct Values {
    all: Vec<Value>,
    ids: HashMap<Value, usize>,
    registers: HashMap<Register, usize>,
}
impl Values {
    fn intern(&mut self, value: Value) -> usize {
        if let Some(&id) = self.ids.get(&value) {
            return id;
        }
        self.all.push(value);
        self.ids.insert(value, self.all.len() - 1);
        self.all.len() - 1
    }
    fn fresh(&mut self) -> usize {
        let n = self.all.len();
        self.intern(Value::Fresh(n))
    }
    fn of(&mut self, register: Register) -> usize {
        let register = register.full_register();
        if let Some(&id) = self.registers.get(&register) {
            return id;
        }
        let id = self.intern(Value::Argument(register));
        self.registers.insert(register, id);
        id
    }
    fn set(&mut self, register: Register, id: usize) {
        self.registers.insert(register.full_register(), id);
    }
    fn load_of(&self, id: usize) -> Option<(usize, usize)> {
        match self.all[id] {
            Value::Load(base, disp) => Some((base, disp)),
            _ => None,
        }
    }
    fn head(&self, id: usize) -> bool {
        self.ids.contains_key(&Value::Load(id, 0)) && self.ids.contains_key(&Value::Load(id, 8))
    }
    fn table(&self, id: usize) -> bool {
        self.load_of(id)
            .is_some_and(|(h, d)| d == 0 && self.head(h))
    }
    fn functions(&self, id: usize) -> bool {
        self.load_of(id).is_some_and(|(e, d)| {
            d == 0 && self.load_of(e).is_some_and(|(h, d)| d == 8 && self.head(h))
        })
    }
}

pub(super) fn trace(image: &Image, entry: usize) -> Option<Vec<Step>> {
    const VOLATILE: [Register; 7] = [
        Register::RAX,
        Register::RCX,
        Register::RDX,
        Register::R8,
        Register::R9,
        Register::R10,
        Register::R11,
    ];
    let mut values = Values::default();
    let mut raw = Vec::new();
    let mut at = entry;
    let mut restored = false;
    for _ in 0..96 {
        let i = scan::instruction(image, at)?;
        at = i.next_ip() as usize;
        let epilogue = std::mem::replace(
            &mut restored,
            i.mnemonic() == Mnemonic::Pop
                || i.mnemonic() == Mnemonic::Add && i.op0_register() == Register::RSP,
        );
        let memory = i.op1_kind() == OpKind::Memory
            && i.memory_index() == Register::None
            && !matches!(i.memory_base(), Register::RIP | Register::None);
        match i.flow_control() {
            FlowControl::Return => break,
            FlowControl::UnconditionalBranch if epilogue || i.ip() as usize == entry => {
                raw.push(Raw::Step(Step::Tail(i.near_branch_target() as usize)));
                break;
            }
            FlowControl::UnconditionalBranch => {
                at = i.near_branch_target() as usize;
                continue;
            }
            FlowControl::IndirectBranch => {
                if i.op0_kind() == OpKind::Register {
                    raw.push(Raw::Register(values.of(i.op0_register())));
                }
                break;
            }
            FlowControl::Call | FlowControl::IndirectCall => {
                match i.op0_kind() {
                    OpKind::NearBranch64 => {
                        raw.push(Raw::Step(Step::Call(i.near_branch_target() as usize)))
                    }
                    OpKind::Register => raw.push(Raw::Register(values.of(i.op0_register()))),
                    OpKind::Memory
                        if !matches!(i.memory_base(), Register::RIP | Register::None) =>
                    {
                        let base = values.of(i.memory_base());
                        raw.push(Raw::CallMemory(base, i.memory_displacement64() as usize));
                    }
                    _ => {}
                }
                for r in VOLATILE {
                    let id = values.fresh();
                    values.set(r, id);
                }
                continue;
            }
            FlowControl::Next | FlowControl::ConditionalBranch => {}
            _ => break,
        }
        if i.mnemonic() == Mnemonic::Mov && i.op0_kind() == OpKind::Register && memory {
            if i.memory_size() != MemorySize::UInt64 {
                let id = values.fresh();
                values.set(i.op0_register(), id);
                continue;
            }
            let base = values.of(i.memory_base());
            let id = values.intern(Value::Load(base, i.memory_displacement64() as usize));
            values.set(i.op0_register(), id);
            raw.push(Raw::Load(id));
        } else if i.mnemonic() == Mnemonic::Mov
            && i.op0_kind() == OpKind::Register
            && i.op1_kind() == OpKind::Register
        {
            let id = values.of(i.op1_register());
            values.set(i.op0_register(), id);
        } else if i.mnemonic() == Mnemonic::Cmp
            && i.op0_kind() == OpKind::Memory
            && i.memory_displacement64() == 0
            && i.op1_kind() == OpKind::Immediate8to32
            && i.immediate8to32() == 0
        {
            raw.push(Raw::Compare(values.of(i.memory_base())));
        } else if i.mnemonic() == Mnemonic::Mov
            && i.op0_kind() == OpKind::Register
            && i.op0_register() == Register::R8D
            && i.op1_kind() == OpKind::Immediate32
        {
            raw.push(Raw::Step(Step::Index(i.immediate32())));
            let id = values.fresh();
            values.set(Register::R8, id);
        } else if i.op0_kind() == OpKind::Register {
            let id = values.fresh();
            values.set(i.op0_register(), id);
        }
    }
    let steps: Vec<Step> = raw
        .into_iter()
        .filter_map(|r| match r {
            Raw::Load(id) => {
                let (base, disp) = values.load_of(id)?;
                (disp != 0 && values.table(base)).then_some(Step::Slot(disp))
            }
            Raw::Compare(base) => values.table(base).then_some(Step::Version),
            Raw::CallMemory(base, disp) => values.functions(base).then_some(Step::Jni(disp)),
            Raw::Register(id) => {
                let (base, disp) = values.load_of(id)?;
                values.functions(base).then_some(Step::Jni(disp))
            }
            Raw::Step(step) => Some(step),
        })
        .collect();
    steps
        .iter()
        .any(|s| matches!(s, Step::Jni(_)))
        .then_some(steps)
}
