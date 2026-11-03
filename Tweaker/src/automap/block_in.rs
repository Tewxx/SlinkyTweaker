use super::Map;

pub fn resolve(map: &mut Map) {
    let initializer = map
        .hook("Block In", super::Event::ModuleState)
        .map(|h| h.callback);
    if let Some(module) = map.modules.get("Block In") {
        let settings_callback = module.sync_callback;
        map.insert("block_in_module", module.descriptor);
        if let Some(rva) = initializer {
            map.insert("block_in_module_init", rva);
        }
        if let Some(rva) = settings_callback {
            map.insert("block_in_settings_init", rva);
        }
    }
}
