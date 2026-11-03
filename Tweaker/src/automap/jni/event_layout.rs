use crate::automap::{image::Image, scan, Event, Map};
use iced_x86::{FlowControl, Instruction, MemorySize, Mnemonic, OpKind, Register};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventLayout {
    pub context_event: usize,
    pub jni_head: usize,
    pub minecraft: usize,
    pub player: usize,
    pub phase: usize,
    pub silent_requested: usize,
    pub silent_move_correct: usize,
    pub player_elapsed: usize,
}

fn prefix(image: &Image, entry: usize) -> Vec<Instruction> {
    let mut result = Vec::new();
    let mut at = entry;
    for _ in 0..128 {
        let Some(inst) = scan::instruction(image, at) else {
            break;
        };
        at = inst.next_ip() as usize;
        let stop = matches!(
            inst.flow_control(),
            FlowControl::Return | FlowControl::IndirectBranch | FlowControl::UnconditionalBranch
        );
        result.push(inst);
        if stop {
            break;
        }
    }
    result
}

fn load(inst: &Instruction, destination: Register, base: Register) -> Option<usize> {
    (inst.mnemonic() == Mnemonic::Mov
        && inst.op0_kind() == OpKind::Register
        && inst.op0_register() == destination
        && inst.op1_kind() == OpKind::Memory
        && inst.memory_base() == base
        && inst.memory_index() == Register::None
        && inst.memory_size() == MemorySize::UInt64)
        .then(|| inst.memory_displacement64() as usize)
}

fn argument_before(
    instructions: &[Instruction],
    index: usize,
    destination: Register,
) -> Option<usize> {
    let mut result = None;
    for inst in instructions[..index].iter().rev().take(8) {
        if inst.flow_control() == FlowControl::Call {
            break;
        }
        if inst.op0_kind() == OpKind::Register && inst.op0_register() == destination {
            result = load(inst, destination, Register::RSI);
            break;
        }
    }
    result.filter(|&offset| offset <= 0x80 && offset % 8 == 0)
}

fn client_tick(image: &Image, map: &mut Map, entry: usize) -> Option<EventLayout> {
    let instructions = prefix(image, entry);
    let calls: Vec<_> = instructions
        .iter()
        .enumerate()
        .filter(|(_, i)| i.flow_control() == FlowControl::Call)
        .collect();
    if calls.len() != 4 {
        return None;
    }
    let roles = [
        super::game_settings::WRAPPER,
        super::attack_binding::WRAPPER,
        super::key_code::WRAPPER,
        super::player_angles::WRAPPER,
    ];
    if !calls
        .iter()
        .zip(roles)
        .all(|((_, i), role)| role.matches(image, i.near_branch_target() as usize))
    {
        return None;
    }
    let after_key = &instructions[calls[2].0 + 1];
    if after_key.mnemonic() != Mnemonic::Cmp
        || after_key.op0_register() != Register::EAX
        || after_key.immediate8to32() != -99
    {
        return None;
    }
    let context_event = instructions
        .iter()
        .find_map(|i| load(i, Register::RSI, Register::RCX))?;
    let jni_head = argument_before(&instructions, calls[0].0, Register::RCX)?;
    let minecraft = argument_before(&instructions, calls[0].0, Register::RDX)?;
    if argument_before(&instructions, calls[3].0, Register::RCX)? != jni_head {
        return None;
    }
    let player = argument_before(&instructions, calls[3].0, Register::RDX)?;
    let phases: Vec<_> = instructions[calls[2].0 + 1..calls[3].0]
        .iter()
        .filter(|i| {
            i.mnemonic() == Mnemonic::Cmp
                && i.op0_kind() == OpKind::Memory
                && i.memory_base() == Register::RSI
                && i.memory_size() == MemorySize::UInt8
                && i.op1_kind() == OpKind::Immediate8
                && i.immediate8() == 1
        })
        .map(|i| i.memory_displacement64() as usize)
        .collect();
    if phases.len() != 1 || phases[0] > 0x80 || context_event != 8 {
        return None;
    }
    for ((_, inst), role) in calls.iter().zip(roles) {
        map.insert(role.name, inst.near_branch_target() as usize);
    }
    Some(EventLayout {
        context_event,
        jni_head,
        minecraft,
        player,
        phase: phases[0],
        silent_requested: 8,
        silent_move_correct: 9,
        player_elapsed: 0,
    })
}

pub fn resolve(image: &Image, map: &mut Map) {
    if !map.modules.contains_key("Auto Clicker") {
        return;
    }
    let tick = map
        .hook("Auto Clicker", Event::ClientTick)
        .map(|h| h.callback);
    if let Some(entry) = tick {
        map.event_layout = client_tick(image, map, entry);
    }
    crate::automap::native_attack_input::resolve(image, map);
    if let Some(aim) = map.hook("Aim Assist", Event::SilentRequest) {
        let writes: Vec<_> = prefix(image, aim.callback)
            .iter()
            .filter(|i| {
                i.mnemonic() == Mnemonic::Mov
                    && i.op0_kind() == OpKind::Memory
                    && i.memory_base() == Register::RCX
                    && i.memory_size() == MemorySize::UInt8
                    && i.op1_kind() == OpKind::Immediate8
                    && i.immediate8() == 1
            })
            .map(|i| i.memory_displacement64() as usize)
            .collect();
        if writes != [8, 9] {
            map.event_layout = None;
        }
    } else {
        map.event_layout = None;
    }
    if let Some(entry) = map
        .hook("Aim Assist", Event::PlayerUpdate)
        .map(|h| h.callback)
    {
        let mut context = Register::None;
        let mut elapsed = std::collections::BTreeSet::new();
        let mut at = entry;
        for _ in 0..1024 {
            let Some(i) = scan::instruction(image, at) else {
                break;
            };
            if context == Register::None
                && i.mnemonic() == Mnemonic::Mov
                && i.op0_kind() == OpKind::Register
                && i.op1_kind() == OpKind::Register
                && i.op1_register() == Register::RCX
            {
                context = i.op0_register();
            }
            if context != Register::None
                && i.mnemonic() == Mnemonic::Movsd
                && i.op1_kind() == OpKind::Memory
                && i.memory_base() == context
                && i.memory_index() == Register::None
                && i.memory_displacement64() <= 0x80
            {
                elapsed.insert(i.memory_displacement64() as usize);
            }
            at = i.next_ip() as usize;
            if i.flow_control() == FlowControl::Return {
                break;
            }
        }
        if elapsed.len() == 1 {
            if let Some(layout) = map.event_layout.as_mut() {
                layout.player_elapsed = *elapsed.first().unwrap();
            }
        } else {
            map.event_layout = None;
        }
    } else {
        map.event_layout = None;
    }
}
