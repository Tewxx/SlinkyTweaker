use super::Wrapper;

pub(super) const WRAPPER: Wrapper = Wrapper {
    name: "jni_game_settings",
    pattern: "55 48 89 E5 48 8B 01 48 8B 49 08 4C 8B 01 4D 8B 88 F8 02 00 00 4C 8B 80 ?? ?? ?? ?? 5D 49 FF E1",
};
