use super::Wrapper;

pub(super) const WRAPPER: Wrapper = Wrapper {
    name: "jni_attack_binding",
    pattern: "55 56 48 83 EC 28 48 8D 6C 24 20 48 89 CE 48 8B 01 48 8B 49 08 4C 8B 09 4C 8B 80 ?? ?? ?? ?? 41 FF 91 F8 02 00 00 48 8B 4E 08 48 8B 11 4C 8B 8A 68 05 00 00 48 89 C2 45 31 C0",
};
