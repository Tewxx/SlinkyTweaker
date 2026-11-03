mod notify_handler;
mod notify_push_signature;
mod push_category_notify;
mod push_simple_notify;

use super::{image::Image, scan, Map};
pub const RING_REQUIRED: &[&str] = &["queue_counter_a", "queue_counter_b", "slot_array"];
pub fn resolve(image: &Image, map: &mut Map) {
    notify_push_signature::resolve(image, map);
    notify_handler::resolve(image, map);
    push_category_notify::resolve(image, map);
    push_simple_notify::resolve(image, map);
    super::format_text::resolve(image, map);
    super::module_manager::enable_batch::resolve(image, map);
    for (name, text) in [
        ("enabled_label", "Enabled"),
        ("disabled_label", "Disabled"),
        ("press_gui_fmt", "Press %s to open GUI"),
        ("hide_hold_notifs", "hide_hold_notifs"),
    ] {
        let hits = image.string_hits(text);
        if hits.len() == 1 {
            map.insert(name, hits[0]);
        }
    }
    if let Some(host) = map.anchors.get("push_category_notify") {
        if let Some(&table) = scan::rip_leas(image, host.rva).first() {
            let valid = image.data(table, 4 * 8, false)
                && image.u64(table + 16) == Some(0)
                && [0, 1, 3].iter().all(|i| {
                    image
                        .u64(table + i * 8)
                        .and_then(|p| image.rva(p as usize))
                        .and_then(|r| image.cstr(r))
                        .is_some()
                });
            if valid {
                map.insert("category_table", table);
            }
        }
    }
    if let Some(host) = map.anchors.get("push_simple_notify") {
        let targets = scan::rip_leas(image, host.rva);
        if targets.len() >= 3 {
            let (a, b, slots) = (targets[0], targets[1], targets[2]);
            let valid = a != b
                && image.data(a, 8, true)
                && image.data(b, 8, true)
                && image.data(slots, 10 * 0x70, true)
                && (0..10).all(|i| {
                    image.u32(slots + i * 0x70).is_some_and(|v| v <= 2)
                        && image
                            .u32(slots + i * 0x70 + 0x6c)
                            .is_some_and(|v| f32::from_bits(v).is_finite())
                });
            if valid {
                for (name, rva) in [
                    ("queue_counter_a", a),
                    ("queue_counter_b", b),
                    ("slot_array", slots),
                ] {
                    map.insert(name, rva);
                }
            }
        }
    }
    if !map.anchors.contains_key("enabled_label") {
        if let Some(host) = map.anchors.get("module_enable_batch") {
            let hits: std::collections::BTreeSet<_> = scan::references(image, host.rva)
                .into_iter()
                .filter(|&r| image.cstr(r) == Some("Enabled"))
                .collect();
            if hits.len() == 1 {
                map.insert("enabled_label", *hits.first().unwrap());
            }
        }
    }
}
