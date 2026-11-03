mod abi;
mod block_at;
mod block_from_item;
mod block_id;
mod block_pos_ints;
mod block_pos_new;
mod bounding_box;
mod bounds;
mod context;
mod current_item;
mod entity_id;
mod fall_distance;
mod fingerprint;
mod focus;
mod get_block;
mod hit_block;
mod hit_entity;
mod hit_side;
mod inventory;
mod key_pressed;
mod members;
mod object_mouse_over;
mod on_ground;
mod ordinal;
mod position;
mod prev_position;
mod ray_trace;
mod set_current_item;
mod sneaking;
mod sprinting;
mod stack_in_slot;
mod stack_item;
mod stack_size;
mod trace;
mod vec3_new;
mod world;

use super::{image::Image, Map};
use fingerprint::Helper;
use std::collections::BTreeSet;
use trace::Step;

const HELPERS: &[Helper] = &[
    ordinal::HELPER,
    world::HELPER,
    focus::HELPER,
    on_ground::HELPER,
    fall_distance::HELPER,
    sprinting::HELPER,
    sneaking::HELPER,
    inventory::HELPER,
    current_item::HELPER,
    set_current_item::HELPER,
    stack_in_slot::HELPER,
    stack_item::HELPER,
    stack_size::HELPER,
    block_from_item::HELPER,
    block_id::HELPER,
    block_pos_new::HELPER,
    get_block::HELPER,
    block_at::HELPER,
    vec3_new::HELPER,
    ray_trace::HELPER,
    hit_side::HELPER,
    block_pos_ints::HELPER,
    hit_block::HELPER,
    position::HELPER,
    bounds::HELPER,
    prev_position::HELPER,
    bounding_box::HELPER,
    entity_id::HELPER,
    hit_entity::HELPER,
    key_pressed::HELPER,
    object_mouse_over::HELPER,
];

pub fn resolve(image: &Image, map: &mut Map) {
    let mut entries = BTreeSet::new();
    let members = members::members(image, &mut entries);
    if members.len() < 100 {
        return;
    }
    let traces: Vec<(usize, Vec<Step>)> = entries
        .into_iter()
        .filter_map(|entry| Some((entry, trace::trace(image, entry)?)))
        .collect();
    for helper in HELPERS {
        let (name, pattern) = (helper.name, helper.pattern);
        let found: Vec<usize> = traces
            .iter()
            .filter(|(_, steps)| fingerprint::matches(steps, pattern, &members, map))
            .map(|(entry, _)| *entry)
            .collect();
        if let [rva] = found[..] {
            map.insert(name, rva);
        }
    }
    if !context::game_update_layout(image, map) {
        map.anchors.remove("native_on_ground");
    }
}
