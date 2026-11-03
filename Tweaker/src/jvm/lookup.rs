use crate::error::{Error, Result};
use super::Method;
use jni_simple::{jclass, jmethodID, JNIEnv};

pub unsafe fn static_method(env: &JNIEnv, class: jclass, m: &Method) -> Result<jmethodID> {
    unsafe {
        for name in m.names {
            for sig in m.sigs {
                let id = env.GetStaticMethodID(class, *name, *sig);
                if !id.is_null() {
                    return Ok(id);
                }
                env.ExceptionClear();
            }
        }
        Err(not_found(m))
    }
}

fn not_found(m: &Method) -> Error {
    let tried = m
        .names
        .iter()
        .flat_map(|n| m.sigs.iter().map(move |s| format!("  {n}{s}")))
        .collect::<Vec<_>>()
        .join("\n");
    Error::NotFound {
        what: m.what.to_string(),
        tried,
    }
}
