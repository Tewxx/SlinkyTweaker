use super::*;

pub(super) fn resolve(
    image: &Image,
    map: &mut Map,
    bodies: &BTreeMap<usize, Vec<Instruction>>,
    witnesses: &BTreeMap<usize, BTreeSet<String>>,
    claim: usize,
) {
    let Some(layout) = map.event_layout else {
        return;
    };
    let required = [
        "native_inventory",
        "native_current_item",
        "native_set_current_item",
    ];
    let [Some(inventory), Some(current), Some(setter)] =
        required.map(|name| map.anchors.get(name).map(|a| a.rva))
    else {
        return;
    };
    let candidates: Vec<_> = bodies
        .iter()
        .filter_map(|(&entry, body)| {
            if body.first()?.ip() as usize != entry
                || witnesses.get(&entry)?.len() < 2
                || ![inventory, current, setter, claim]
                    .into_iter()
                    .all(|target| calls(body, target))
            {
                return None;
            }
            let context = body
                .iter()
                .take(24)
                .find(|i| {
                    i.mnemonic() == Mnemonic::Mov
                        && i.op1_kind() == OpKind::Register
                        && i.op1_register() == Register::RCX
                        && i.op0_register() != Register::RCX
                })?
                .op0_register();
            let context_player = body.iter().any(|i| {
                i.mnemonic() == Mnemonic::Mov
                    && i.op0_register() == Register::RDX
                    && i.op1_kind() == OpKind::Memory
                    && i.memory_base() == context
                    && i.memory_displacement64() as usize == layout.player
            });
            let context_head = body.iter().any(|i| {
                i.mnemonic() == Mnemonic::Mov
                    && i.op0_register() == Register::RCX
                    && i.op1_kind() == OpKind::Memory
                    && i.memory_base() == context
                    && i.memory_displacement64() as usize == layout.jni_head
            });
            let force = body.iter().any(|i| {
                i.mnemonic() == Mnemonic::Test
                    && i.op0_register() == Register::R8L
                    && i.op1_register() == Register::R8L
            });
            let callback = body.iter().take(24).any(|i| {
                i.mnemonic() == Mnemonic::Mov
                    && i.op0_kind() == OpKind::Memory
                    && i.op1_register() == Register::R9
            });
            let latches: BTreeSet<_> = body
                .iter()
                .filter(|i| {
                    i.mnemonic() == Mnemonic::Mov
                        && writes_global(i)
                        && i.op1_kind() == OpKind::Register
                        && i.memory_size() == iced_x86::MemorySize::UInt64
                })
                .filter_map(global)
                .filter(|&at| image.data(at, 8, true))
                .collect();
            if !context_player || !context_head || !force || !callback || latches.len() != 1 {
                return None;
            }
            Some((entry, *latches.first().unwrap()))
        })
        .collect();
    let [(implementation, latch)] = candidates.as_slice() else {
        return;
    };
    let thunks: Vec<_> = bodies
        .keys()
        .filter_map(|&entry| {
            if thunk_target(image, entry) != Some(*implementation) {
                return None;
            }
            let mut at = entry;
            let mut scorer = None;
            for _ in 0..12 {
                let i = scan::instruction(image, at)?;
                if i.mnemonic() == Mnemonic::Lea
                    && i.op0_register() == Register::R9
                    && i.is_ip_rel_memory_operand()
                {
                    scorer = Some(i.ip_rel_memory_address() as usize);
                }
                if i.flow_control() == FlowControl::UnconditionalBranch {
                    break;
                }
                at = i.next_ip() as usize;
            }
            let scorer = scorer.filter(|&score| block_score::valid(image, score))?;
            Some((entry, scorer))
        })
        .collect();
    let [(entry, scorer)] = thunks.as_slice() else {
        return;
    };
    for (name, rva) in [
        ("hotbar_select_block", *entry),
        ("hotbar_block_score", *scorer),
        ("hotbar_select_latch", *latch),
    ] {
        map.insert(name, rva);
    }
}
