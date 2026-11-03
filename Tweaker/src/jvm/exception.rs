use crate::error::{Error, Result};
use jni_simple::JNIEnv;

pub unsafe fn take(env: &JNIEnv) -> Option<String> {
    unsafe {
        if !env.ExceptionCheck() {
            return None;
        }
        let throwable = env.ExceptionOccurred();
        env.ExceptionClear();
        if throwable.is_null() {
            return Some("<unknown exception>".into());
        }

        let class = env.GetObjectClass(throwable);
        let to_string = env.GetMethodID(class, "toString", "()Ljava/lang/String;");
        if to_string.is_null() {
            env.ExceptionClear();
            return Some("<exception with no toString()>".into());
        }
        let text = env.CallObjectMethod0(throwable, to_string);
        Some(super::strings::to_rust(env, text))
    }
}

pub unsafe fn check(env: &JNIEnv, during: &str) -> Result<()> {
    unsafe {
        match take(env) {
            None => Ok(()),
            Some(message) => Err(Error::JavaException {
                during: during.to_string(),
                message,
            }),
        }
    }
}
