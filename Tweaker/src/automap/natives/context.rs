use crate::automap::{image::Image, scan, Event, Map};
use iced_x86::{FlowControl, MemorySize, Mnemonic, OpKind, Register};
use std::collections::HashMap;

pub(super) fn game_update_layout(image: &Image, map: &Map) -> bool {
    let (Some(layout), Some(hook), Some(on_ground)) = (
        map.event_layout,
        map.hook("Block In", Event::GameUpdate),
        map.anchors.get("native_on_ground"),
    ) else {
        return false;
    };
    let mut context = None;
    let mut arguments: HashMap<Register, usize> = HashMap::new();
    let mut at = hook.callback;
    for _ in 0..64 {
        let Some(i) = scan::instruction(image, at) else {
            return false;
        };
        at = i.next_ip() as usize;
        let load = i.mnemonic() == Mnemonic::Mov
            && i.op0_kind() == OpKind::Register
            && i.op1_kind() == OpKind::Memory
            && i.memory_index() == Register::None
            && i.memory_size() == MemorySize::UInt64;
        if load && context.is_none() && i.memory_base() == Register::RCX {
            (i.memory_displacement64() as usize == layout.context_event)
                .then(|| context = Some(i.op0_register()));
        } else if load && Some(i.memory_base()) == context {
            arguments.insert(i.op0_register(), i.memory_displacement64() as usize);
        }
        match i.flow_control() {
            FlowControl::Call if i.near_branch_target() as usize == on_ground.rva => {
                return arguments.get(&Register::RCX) == Some(&layout.jni_head)
                    && arguments.get(&Register::RDX) == Some(&layout.player);
            }
            FlowControl::Next | FlowControl::ConditionalBranch => {}
            _ => return false,
        }
    }
    false
}
