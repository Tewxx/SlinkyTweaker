use crate::error::{Error, Result};
use jni_simple::{jclass, jmethodID, jobject, JNIEnv};

pub struct Reflect {
    class_get_name: jmethodID,
    class_get_methods: jmethodID,
    method_get_name: jmethodID,
    method_get_return_type: jmethodID,
}

impl Reflect {
    pub unsafe fn new(env: &JNIEnv) -> Result<Self> {
        unsafe {
            let class = env.FindClass("java/lang/Class");
            let method = env.FindClass("java/lang/reflect/Method");
            if class.is_null() || method.is_null() {
                env.ExceptionClear();
                return Err(Error::NotFound {
                    what: "java.lang.reflect.*".into(),
                    tried: String::new(),
                });
            }

            Ok(Self {
                class_get_name: env.GetMethodID(class, "getName", "()Ljava/lang/String;"),
                class_get_methods: env.GetMethodID(
                    class,
                    "getMethods",
                    "()[Ljava/lang/reflect/Method;",
                ),
                method_get_name: env.GetMethodID(method, "getName", "()Ljava/lang/String;"),
                method_get_return_type: env.GetMethodID(
                    method,
                    "getReturnType",
                    "()Ljava/lang/Class;",
                ),
            })
        }
    }

    pub unsafe fn class_name(&self, env: &JNIEnv, class: jclass) -> String {
        unsafe {
            let name = env.CallObjectMethod0(class, self.class_get_name);
            super::strings::to_rust(env, name)
        }
    }

    pub unsafe fn type_name_of(&self, env: &JNIEnv, object: jobject) -> String {
        unsafe { self.class_name(env, env.GetObjectClass(object)) }
    }

    pub unsafe fn method_names_returning(
        &self,
        env: &JNIEnv,
        class: jclass,
        type_name: &str,
    ) -> Vec<String> {
        unsafe {
            let mut out = Vec::new();
            let methods = env.CallObjectMethod0(class, self.class_get_methods);
            if methods.is_null() {
                env.ExceptionClear();
                return out;
            }
            for i in 0..env.GetArrayLength(methods) {
                let m = env.GetObjectArrayElement(methods, i);
                let returns = env.CallObjectMethod0(m, self.method_get_return_type);
                if self.class_name(env, returns) == type_name {
                    let name = env.CallObjectMethod0(m, self.method_get_name);
                    out.push(super::strings::to_rust(env, name));
                }
            }
            out
        }
    }

}
