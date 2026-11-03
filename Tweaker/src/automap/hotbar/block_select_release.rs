use super::*;

pub(super) fn resolve(
    image: &Image,
    map: &mut Map,
    bodies: &BTreeMap<usize, Vec<Instruction>>,
    release: usize,
) {
    let Some(latch) = map.anchors.get("hotbar_select_latch").map(|a| a.rva) else {
        return;
    };
    let mut candidates: BTreeSet<_> = BTreeSet::new();
    let extra: BTreeMap<_, _> = small_functions(image, 0x400)
        .into_iter()
        .filter_map(|entry| {
            let mut at = entry;
            let mut found = false;
            for _ in 0..16 {
                let Some(i) = scan::instruction(image, at) else {
                    break;
                };
                if i.mnemonic() == Mnemonic::Mov
                    && i.op0_register() == Register::RCX
                    && global(&i) == Some(latch)
                {
                    found = true;
                    break;
                }
                if i.flow_control() != FlowControl::Next {
                    break;
                }
                at = i.next_ip() as usize;
            }
            found.then(|| (entry, scan::function_body(image, entry)))
        })
        .collect();
    for (&entry, body) in bodies.iter().chain(extra.iter()) {
        let candidate = (|| {
            if body.first()?.ip() as usize != entry || body.len() > 80 || !calls(body, release) {
                return None;
            }
            let load = body.iter().any(|i| {
                i.mnemonic() == Mnemonic::Mov
                    && i.op0_register() == Register::RCX
                    && i.op1_kind() == OpKind::Memory
                    && global(i) == Some(latch)
            });
            let clear = body.iter().any(|i| {
                i.mnemonic() == Mnemonic::Mov
                    && writes_global(i)
                    && global(i) == Some(latch)
                    && i.op1_kind() == OpKind::Immediate32to64
                    && i.immediate32to64() == 0
            });
            let only_release = body
                .iter()
                .filter(|i| i.flow_control() == FlowControl::Call)
                .all(|i| i.near_branch_target() as usize == release);
            (load && clear && only_release && image.executable(entry)).then_some(entry)
        })();
        if let Some(entry) = candidate {
            candidates.insert(entry);
        }
    }
    if candidates.len() == 1 {
        map.insert("hotbar_select_release", *candidates.first().unwrap());
    }
}
