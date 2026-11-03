use super::{image::Image, scan};
use iced_x86::{FlowControl, MemorySize, Mnemonic, OpKind, Register};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Key {
    Image(usize),
    Digit(u8),
}
impl Key {
    pub fn text(self, image: &Image) -> Option<String> {
        match self {
            Self::Image(at) => image.cstr(at).map(str::to_owned),
            Self::Digit(digit) => Some(char::from(digit).to_string()),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Call {
    pub key: Key,
    pub descriptor: usize,
    pub helper: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct State {
    at: usize,
    key: Option<Key>,
    descriptor: Option<usize>,
    local: Option<(Register, u64, u8)>,
    alias: Option<(Register, Register, u64)>,
}

pub fn calls(image: &Image, entry: usize) -> Vec<Call> {
    let mut pending = VecDeque::from([State {
        at: entry,
        key: None,
        descriptor: None,
        local: None,
        alias: None,
    }]);
    let mut visited = BTreeSet::new();
    let mut calls = BTreeSet::new();
    while let Some(mut state) = pending.pop_front() {
        for _ in 0..4096 {
            if visited.len() >= 8192 || state.at.abs_diff(entry) > 0x10000 || !visited.insert(state)
            {
                break;
            }
            let Some(inst) = scan::instruction(image, state.at) else {
                break;
            };
            let rip = (inst.mnemonic() == Mnemonic::Lea && inst.is_ip_rel_memory_operand())
                .then(|| inst.ip_rel_memory_address() as usize);
            let frame = matches!(inst.memory_base(), Register::RBP | Register::RSP)
                && inst.memory_index() == Register::None;
            if inst.mnemonic() == Mnemonic::Mov && inst.op0_kind() == OpKind::Memory && frame {
                state.local = if inst.op1_kind() == OpKind::Immediate16
                    && inst.memory_size() == MemorySize::UInt16
                    && (b'1' as u16..=b'9' as u16).contains(&inst.immediate16())
                {
                    Some((
                        inst.memory_base(),
                        inst.memory_displacement64(),
                        inst.immediate16() as u8,
                    ))
                } else {
                    None
                };
            }
            if inst.op0_kind() == OpKind::Register
                && !matches!(
                    inst.mnemonic(),
                    Mnemonic::Cmp | Mnemonic::Test | Mnemonic::Push
                )
            {
                let dst = inst.op0_register();
                if state.alias.is_some_and(|a| a.0 == dst) {
                    state.alias = None;
                }
                if inst.mnemonic() == Mnemonic::Lea
                    && frame
                    && matches!(
                        dst,
                        Register::RDI
                            | Register::RSI
                            | Register::RBX
                            | Register::R12
                            | Register::R13
                            | Register::R14
                            | Register::R15
                    )
                {
                    state.alias = Some((dst, inst.memory_base(), inst.memory_displacement64()));
                }
                match dst {
                    Register::RDX | Register::EDX | Register::DX | Register::DL => {
                        state.key = rip.map(Key::Image);
                        let local = if inst.mnemonic() == Mnemonic::Lea && frame {
                            Some((inst.memory_base(), inst.memory_displacement64()))
                        } else if inst.mnemonic() == Mnemonic::Mov
                            && inst.op1_kind() == OpKind::Register
                        {
                            state
                                .alias
                                .filter(|a| a.0 == inst.op1_register())
                                .map(|a| (a.1, a.2))
                        } else {
                            None
                        };
                        if let (Some((base, offset)), Some((b, o, digit))) = (local, state.local) {
                            if base == b && offset == o {
                                state.key = Some(Key::Digit(digit));
                            }
                        }
                    }
                    Register::R8 | Register::R8D | Register::R8W | Register::R8L => {
                        state.descriptor = rip
                    }
                    _ => (),
                }
            }
            let next = inst.next_ip() as usize;
            match inst.flow_control() {
                FlowControl::Call | FlowControl::UnconditionalBranch => {
                    let helper = inst.near_branch_target() as usize;
                    if inst.flow_control() == FlowControl::UnconditionalBranch
                        && helper >= entry
                        && helper.abs_diff(entry) <= 0x10000
                    {
                        state.at = helper;
                        pending.push_back(state);
                        break;
                    }
                    if let (Some(key), Some(descriptor)) = (state.key, state.descriptor) {
                        if key.text(image).is_some()
                            && image.data(descriptor, 0x20, true)
                            && image.executable(helper)
                        {
                            calls.insert(Call {
                                key,
                                descriptor,
                                helper,
                            });
                        }
                    }
                    if inst.flow_control() == FlowControl::UnconditionalBranch {
                        break;
                    }
                    state.key = None;
                    state.descriptor = None;
                }
                FlowControl::ConditionalBranch => {
                    let mut branch = state;
                    branch.at = inst.near_branch_target() as usize;
                    pending.push_back(branch);
                }
                FlowControl::Return | FlowControl::IndirectBranch | FlowControl::Exception => break,
                FlowControl::IndirectCall => {
                    state.key = None;
                    state.descriptor = None;
                }
                _ => (),
            }
            state.at = next;
        }
    }
    calls.into_iter().collect()
}
