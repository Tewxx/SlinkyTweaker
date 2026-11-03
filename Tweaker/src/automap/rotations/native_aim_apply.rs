use crate::automap::{image::Image, scan, Map};

pub(super) fn resolve(image: &Image, map: &mut Map, callback: &[iced_x86::Instruction]) {
    use iced_x86::{FlowControl, Mnemonic, OpKind, Register};
    let mut candidates = std::collections::BTreeSet::new();
    for i in callback
        .iter()
        .filter(|i| i.flow_control() == FlowControl::Call)
    {
        let target = i.near_branch_target() as usize;
        if !scan::matches_at(
            image,
            target,
            "55 56 57 48 81 EC C0 00 00 00 48 8D AC 24 80 00 00 00",
        ) {
            continue;
        }
        let instructions = scan::function_body(image, target);
        let first: Vec<_> = instructions.iter().take(35).collect();
        let packed = first.iter().any(|i| {
            i.mnemonic() == Mnemonic::Shr
                && i.op0_register() == Register::RCX
                && i.immediate8() == 32
        }) && first.iter().any(|i| {
            i.mnemonic() == Mnemonic::Movd
                && i.op1_kind() == OpKind::Register
                && i.op1_register() == Register::ECX
        });
        let elapsed = first.iter().any(|i| {
            i.mnemonic() == Mnemonic::Movaps
                && i.op1_kind() == OpKind::Register
                && i.op1_register() == Register::XMM1
        });
        let applies = instructions.iter().any(|i| {
            i.flow_control() == FlowControl::UnconditionalBranch
                && map
                    .anchors
                    .get("rotation_add")
                    .is_some_and(|a| a.rva == i.near_branch_target() as usize)
        });
        let settings = [("mode", 0), ("hor_speed", 0), ("ver_speed", 0), ("fov", 0)];
        let reads = settings.iter().all(|(key, _)| {
            map.setting("Aim Assist", key)
                .and_then(|s| s.value_offset.map(|v| s.descriptor + v))
                .is_some_and(|value| {
                    instructions.iter().any(|i| {
                        i.is_ip_rel_memory_operand() && i.ip_rel_memory_address() as usize == value
                    })
                })
        });
        if packed && elapsed && applies && reads {
            candidates.insert(target);
        }
    }
    if candidates.len() == 1 {
        map.insert("native_aim_apply", *candidates.first().unwrap());
    }
}
