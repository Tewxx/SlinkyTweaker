use crate::error::{Error, Result};
use crate::jvm::{self, classloader, exception, lookup, Reflect};
use super::mappings;
use jni_simple::{jclass, jfieldID, jmethodID, jobject, JNIEnv, JavaVM};
pub(crate) mod bed_plates;
pub(crate) mod world;
mod cache;
pub(crate) mod game;

pub struct Client {
    env: JNIEnv,
    _vm: JavaVM,
    loader: jobject,
    instance: jobject,
}

impl Client {
    pub fn attach() -> Result<Self> {
        unsafe {
            let vm = jvm::current_vm()?;
            let env = jvm::attach_current_thread(&vm)?;
            let reflect = Reflect::new(&env)?;

            let (loader, class) = classloader::find_game_class_loader(
                &env,
                &reflect,
                mappings::MINECRAFT_CLASS_CANDIDATES,
            )?;

            let get_instance = lookup::static_method(&env, class, &mappings::GET_MINECRAFT)
                .map_err(|e| describe_lookup_failure(&env, &reflect, class, e))?;

            let instance = env.CallStaticObjectMethod0(class, get_instance);
            exception::check(&env, mappings::GET_MINECRAFT.what)?;
            if instance.is_null() {
                return Err(Error::UnexpectedNull {
                    what: mappings::GET_MINECRAFT.what.to_string(),
                });
            }
            let loader = env.NewGlobalRef(loader);
            let instance = env.NewGlobalRef(instance);
            if loader.is_null() || instance.is_null() {
                return Err(Error::Jvm(
                    "failed to create persistent Minecraft JNI references".into(),
                ));
            }

            Ok(Self {
                env,
                _vm: vm,
                loader,
                instance,
            })
        }
    }
}

unsafe fn find_field(
    env: &JNIEnv,
    class: jclass,
    names: &[&str],
    signatures: &[&str],
    what: &str,
) -> Result<jfieldID> {
    unsafe {
        for name in names {
            for signature in signatures {
                let field = env.GetFieldID(class, *name, *signature);
                if !field.is_null() {
                    return Ok(field);
                }
                env.ExceptionClear();
            }
        }
        Err(Error::NotFound {
            what: what.into(),
            tried: names.join(", "),
        })
    }
}

unsafe fn find_method(
    env: &JNIEnv,
    class: jclass,
    names: &[&str],
    signatures: &[&str],
    what: &str,
) -> Result<jmethodID> {
    unsafe {
        for name in names {
            for signature in signatures {
                let method = env.GetMethodID(class, *name, *signature);
                if !method.is_null() {
                    return Ok(method);
                }
                env.ExceptionClear();
            }
        }
        Err(Error::NotFound {
            what: what.into(),
            tried: names.join(", "),
        })
    }
}

unsafe fn find_static_method(
    env: &JNIEnv,
    class: jclass,
    names: &[&str],
    signatures: &[&str],
    what: &str,
) -> Result<jmethodID> {
    unsafe {
        for name in names {
            for signature in signatures {
                let method = env.GetStaticMethodID(class, *name, *signature);
                if !method.is_null() {
                    return Ok(method);
                }
                env.ExceptionClear();
            }
        }
        Err(Error::NotFound {
            what: what.into(),
            tried: names.join(", "),
        })
    }
}

unsafe fn load_first_class(
    env: &JNIEnv,
    loader: jobject,
    names: &[&str],
    what: &str,
) -> Result<jclass> {
    unsafe {
        for name in names {
            if let Ok(class) = classloader::load_class(env, loader, name) {
                return Ok(class);
            }
        }
        Err(Error::NotFound {
            what: what.into(),
            tried: names.join(", "),
        })
    }
}

unsafe fn describe_lookup_failure(
    env: &JNIEnv,
    reflect: &Reflect,
    class: jclass,
    original: Error,
) -> Error {
    unsafe {
        let name = reflect.class_name(env, class);
        let candidates = reflect
            .method_names_returning(env, class, &name)
            .join("\n  ");
        Error::NotFound {
            what: mappings::GET_MINECRAFT.what.to_string(),
            tried: format!(
                "{}\n\nThe class is really named:\n  {name}\n\n\
                 Static methods returning it (getMinecraft is one of these):\n  {candidates}",
                match original {
                    Error::NotFound { tried, .. } => tried,
                    other => other.to_string(),
                }
            ),
        }
    }
}
