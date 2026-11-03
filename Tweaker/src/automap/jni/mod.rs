mod attack_binding;
mod event_layout;
mod game_settings;
mod key_code;
mod player_angles;

pub use event_layout::{resolve, EventLayout};

#[derive(Clone, Copy)]
struct Wrapper {
    name: &'static str,
    pattern: &'static str,
}

impl Wrapper {
    fn matches(self, image: &super::image::Image, at: usize) -> bool {
        super::scan::matches_at(image, at, self.pattern)
    }
}
