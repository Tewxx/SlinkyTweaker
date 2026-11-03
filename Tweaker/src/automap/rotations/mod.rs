mod camera_override_active;
mod native_aim_apply;
mod native_angle_magnitude;
mod native_angles_to;
mod native_wrap_yaw;
mod packed_angle_add;
mod packed_angle_sub;
mod rotation_add;
mod rotation_claim;
mod rotation_current;
mod rotation_stored;

use crate::automap::{image::Image, scan, Event, Map};
use iced_x86::{FlowControl, Instruction};
use std::collections::BTreeSet;

pub fn resolve(image: &Image, map: &mut Map) {
    rotation_claim::resolve(image, map);
    rotation_add::resolve(image, map);
    rotation_current::resolve(image, map);
    packed_angle_add::resolve(image, map);
    packed_angle_sub::resolve(image, map);
    if let Some(entry) = map
        .hook("Aim Assist", Event::PlayerUpdate)
        .map(|h| h.callback)
    {
        let callback = scan::function_body(image, entry);
        native_aim_apply::resolve(image, map, &callback);
        native_angles_to::resolve(image, map, &callback);
        native_angle_magnitude::resolve(image, map, &callback);
        native_wrap_yaw::resolve(image, map, &callback);
    }
}

pub(super) fn resolve_getters(image: &Image, map: &mut Map, entry: usize) {
    camera_override_active::resolve(image, map, entry);
    rotation_stored::resolve(image, map, entry);
}

fn resolve_callee(
    image: &Image,
    map: &mut Map,
    callback: &[Instruction],
    name: &str,
    pattern: &str,
) {
    let targets: BTreeSet<_> = callback
        .iter()
        .filter(|i| i.flow_control() == FlowControl::Call)
        .map(|i| i.near_branch_target() as usize)
        .filter(|&target| scan::matches_at(image, target, pattern))
        .collect();
    if targets.len() == 1 {
        map.insert(name, *targets.first().unwrap());
    }
}

fn resolve_getter(
    image: &Image,
    map: &mut Map,
    entry: usize,
    name: &str,
    pattern: &str,
    exclude: Option<usize>,
) {
    let mut candidates = BTreeSet::new();
    let mut at = entry;
    for _ in 0..1024 {
        let Some(i) = scan::instruction(image, at) else {
            break;
        };
        if i.flow_control() == FlowControl::Call {
            let target = i.near_branch_target() as usize;
            if Some(target) != exclude && scan::matches_at(image, target, pattern) {
                candidates.insert(target);
            }
        }
        at = i.next_ip() as usize;
        if i.flow_control() == FlowControl::Return {
            break;
        }
    }
    if candidates.len() == 1 {
        map.insert(name, *candidates.first().unwrap());
    }
}
