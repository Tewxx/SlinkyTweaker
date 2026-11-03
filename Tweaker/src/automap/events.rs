mod click_pre;
mod client_tick;
mod game_update;
mod inbound_packet;
mod input;
mod module_state;
mod outbound_packet;
mod overlay_render;
mod packet_send;
mod player_update;
mod raytrace;
mod silent_request;
mod ui_render;
mod world_render;

use super::Map;
use std::collections::{BTreeMap, BTreeSet};

pub const SLOT_COUNT: usize = 27;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    ClientTick,
    PlayerUpdate,
    InboundPacket,
    OutboundPacket,
    ModuleState,
    WorldRender,
    OverlayRender,
    Raytrace,
    UiRender,
    SilentRequest,
    ClickPre,
    Input,
    GameUpdate,
    PacketSend,
}

#[rustfmt::skip]
const REFERENCE: &[(Event, &[(&str, i32)])] = &[
    (Event::ClientTick, client_tick::LISTENERS),
    (Event::PlayerUpdate, player_update::LISTENERS),
    (Event::InboundPacket, inbound_packet::LISTENERS),
    (Event::OutboundPacket, outbound_packet::LISTENERS),
    (Event::ModuleState, module_state::LISTENERS),
    (Event::WorldRender, world_render::LISTENERS),
    (Event::OverlayRender, overlay_render::LISTENERS),
    (Event::Raytrace, raytrace::LISTENERS),
    (Event::UiRender, ui_render::LISTENERS),
    (Event::SilentRequest, silent_request::LISTENERS),
    (Event::ClickPre, click_pre::LISTENERS),
    (Event::Input, input::LISTENERS),
    (Event::GameUpdate, game_update::LISTENERS),
    (Event::PacketSend, packet_send::LISTENERS),
];

const MIN_SCORE: f64 = 0.5;
const MIN_MARGIN: f64 = 0.25;

fn similarity(expected: &[(&str, i32)], live: &BTreeSet<(String, i32)>) -> f64 {
    let expected: BTreeSet<(String, i32)> =
        expected.iter().map(|&(m, p)| (m.to_string(), p)).collect();
    let names =
        |s: &BTreeSet<(String, i32)>| s.iter().map(|(m, _)| m.clone()).collect::<BTreeSet<_>>();
    let (en, ln) = (names(&expected), names(live));
    let union = expected.union(live).count() + en.union(&ln).count();
    if union == 0 {
        return 0.0;
    }
    (expected.intersection(live).count() + en.intersection(&ln).count()) as f64 / union as f64
}

pub fn resolve(map: &mut Map) {
    let mut live = vec![BTreeSet::new(); SLOT_COUNT];
    for module in map.modules.values() {
        for hook in module.hooks.iter().filter(|h| h.slot < SLOT_COUNT) {
            live[hook.slot].insert((module.name.clone(), hook.priority));
        }
    }
    let mut found = BTreeMap::new();
    for &(event, expected) in REFERENCE {
        let mut scores: Vec<(f64, usize)> = live
            .iter()
            .enumerate()
            .map(|(slot, listeners)| (similarity(expected, listeners), slot))
            .collect();
        scores.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (best, slot) = scores[0];
        if best >= MIN_SCORE && best - scores[1].0 >= MIN_MARGIN {
            found.insert(event, slot);
        }
    }
    let mut owners: BTreeMap<usize, Vec<Event>> = BTreeMap::new();
    for (&event, &slot) in &found {
        owners.entry(slot).or_default().push(event);
    }
    for (_, events) in owners.into_iter().filter(|(_, e)| e.len() > 1) {
        for event in events {
            found.remove(&event);
        }
    }
    map.events = found;
}
