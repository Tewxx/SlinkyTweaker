use jni_simple::{jstring, JNIEnv};

pub unsafe fn to_rust(env: &JNIEnv, js: jstring) -> String {
    unsafe {
        if js.is_null() {
            return "<null>".to_string();
        }
        env.GetStringUTFChars_as_string(js)
            .unwrap_or_else(|| "<null>".to_string())
    }
}
