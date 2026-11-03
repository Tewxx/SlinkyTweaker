use crate::error::{Error, Result};
use crate::jvm::Reflect;
use jni_simple::{jclass, jobject, JNIEnv};

const LOADER_HINTS: &[&str] = &[
    "launchwrapper.LaunchClassLoader",
    "LaunchClassLoader",
    "TransformingClassLoader",
    "RelaunchClassLoader",
    "KnotClassLoader",
];

fn loader_rank(name: &str) -> usize {
    LOADER_HINTS
        .iter()
        .position(|hint| name.contains(hint))
        .unwrap_or(LOADER_HINTS.len())
}

pub unsafe fn find_game_class_loader(
    env: &JNIEnv,
    reflect: &Reflect,
    class_names: &[&str],
) -> Result<(jobject, jclass)> {
    unsafe {
        let loaders = collect_thread_loaders(env, reflect)?;
        let mut ranked: Vec<(usize, jobject, String)> = loaders
            .into_iter()
            .map(|(loader, name)| (loader_rank(&name), loader, name))
            .collect();
        ranked.sort_by_key(|(rank, _, name)| (*rank, name.clone()));

        let seen = ranked
            .iter()
            .map(|(_, _, name)| name.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        for (_, loader, _) in ranked {
            for class_name in class_names {
                if let Some(class) = try_load_class(env, loader, class_name) {
                    return Ok((loader, class));
                }
            }
        }

        Err(Error::Jvm(format!(
            "Couldn't find a class loader that exposes Minecraft.\n\
             Tried class names: {}\n\nClass loaders on live threads:\n{}",
            class_names.join(", "),
            seen
        )))
    }
}

pub unsafe fn load_class(env: &JNIEnv, loader: jobject, name: &str) -> Result<jclass> {
    unsafe {
        let loader_class = env.FindClass("java/lang/ClassLoader");
        let load = env.GetMethodID(
            loader_class,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
        );

        let jname = env.NewStringUTF(name);
        let class = env.CallObjectMethod1(loader, load, jname);
        super::exception::check(env, "ClassLoader.loadClass()")?;

        if class.is_null() {
            return Err(Error::NotFound {
                what: format!("class \"{name}\""),
                tried: String::new(),
            });
        }
        Ok(class)
    }
}

unsafe fn try_load_class(env: &JNIEnv, loader: jobject, name: &str) -> Option<jclass> {
    unsafe {
        if let Ok(class) = load_class(env, loader, name) {
            return Some(class);
        }
        env.ExceptionClear();

        let class_class = env.FindClass("java/lang/Class");
        if class_class.is_null() {
            env.ExceptionClear();
            return None;
        }
        let for_name = env.GetStaticMethodID(
            class_class,
            "forName",
            "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;",
        );
        if for_name.is_null() {
            env.ExceptionClear();
            return None;
        }

        let jname = env.NewStringUTF(name);
        let loaded = env.CallStaticObjectMethod3(class_class, for_name, jname, 1, loader);
        if env.ExceptionCheck() {
            env.ExceptionClear();
            return None;
        }
        if loaded.is_null() {
            return None;
        }
        Some(loaded)
    }
}

unsafe fn collect_thread_loaders(
    env: &JNIEnv,
    reflect: &Reflect,
) -> Result<Vec<(jobject, String)>> {
    unsafe {
        let thread = env.FindClass("java/lang/Thread");
        let get_all = env.GetStaticMethodID(thread, "getAllStackTraces", "()Ljava/util/Map;");
        let get_loader =
            env.GetMethodID(thread, "getContextClassLoader", "()Ljava/lang/ClassLoader;");

        let map = env.CallStaticObjectMethod0(thread, get_all);
        super::exception::check(env, "Thread.getAllStackTraces()")?;

        let map_class = env.FindClass("java/util/Map");
        let key_set = env.GetMethodID(map_class, "keySet", "()Ljava/util/Set;");
        let set = env.CallObjectMethod0(map, key_set);

        let set_class = env.FindClass("java/util/Set");
        let to_array = env.GetMethodID(set_class, "toArray", "()[Ljava/lang/Object;");
        let threads = env.CallObjectMethod0(set, to_array);
        super::exception::check(env, "collecting live threads")?;

        let mut loaders: Vec<(jobject, String)> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();

        for i in 0..env.GetArrayLength(threads) {
            let t = env.GetObjectArrayElement(threads, i);
            let loader = env.CallObjectMethod0(t, get_loader);
            if loader.is_null() {
                continue;
            }

            let name = reflect.type_name_of(env, loader);
            if seen.insert(name.clone()) {
                loaders.push((loader, name));
            }
        }

        Ok(loaders)
    }
}
