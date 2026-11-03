use super::Wrapper;

pub(super) const WRAPPER: Wrapper = Wrapper {
    name: "jni_key_code",
    pattern: "55 48 89 E5 48 8B 01 48 8B 49 08 4C 8B 01 4D 8B 88 20 03 00 00 4C 8B 80 ?? ?? ?? ?? 5D 49 FF E1",
};
