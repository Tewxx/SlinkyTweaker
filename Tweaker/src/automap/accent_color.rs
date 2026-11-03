use super::Map;
pub fn resolve(map: &mut Map) {
    map.alias_setting("accent_color", "GUI", "accent_color");
    if let Some(setting) = map
        .setting("GUI", "accent_color")
        .filter(|s| s.kind == "color" && s.value_offset == Some(8))
    {
        map.insert("accent_color_value", setting.descriptor + 8);
    }
}
