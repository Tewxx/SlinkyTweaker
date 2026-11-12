use crate::jvm::{self, classloader, lookup, Reflect};
use crate::minecraft::mappings;
use jni_simple::{jclass, jmethodID, jobject, JNIEnv, JavaVMAttachArgs, JNI_VERSION_1_8};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) type Face = [[u8; 3]; 64];

#[derive(Clone, Default)]
pub(super) struct Account {
    pub name: Option<String>,
    pub face: Option<Face>,
}

static ACCOUNT: Mutex<Account> = Mutex::new(Account { name: None, face: None });
static STARTED: AtomicBool = AtomicBool::new(false);
static LICENSE: Mutex<Option<(Instant, Option<u64>)>> = Mutex::new(None);

const TIMEOUT_MS: i32 = 6000;
const LIFETIME_DAYS: u64 = 3650;

pub(super) fn account() -> Account {
    if !STARTED.swap(true, Ordering::AcqRel) {
        let _ = std::thread::Builder::new().name("tweaker-account".into()).spawn(|| unsafe { fetch() });
    }
    ACCOUNT.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub(super) fn license() -> Option<String> {
    let mut cached = LICENSE.lock().unwrap_or_else(|e| e.into_inner());
    let expiry = match *cached {
        Some((read, expiry)) if read.elapsed() < Duration::from_secs(30) => expiry,
        _ => {
            let expiry = std::fs::read_to_string(std::env::temp_dir().join("SlinkyTweaker").join("license"))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok());
            *cached = Some((Instant::now(), expiry));
            expiry
        }
    }?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let left = expiry.saturating_sub(now);
    Some(match left {
        0 => "Expired".into(),
        s if s / 86_400 >= LIFETIME_DAYS => "Lifetime".into(),
        s if s > 48 * 3600 => format!("{} days left", (s as f64 / 86_400.0).round()),
        s if s > 2 * 3600 => format!("{} hours left", (s as f64 / 3600.0).round()),
        s => format!("{} minutes left", (s as f64 / 60.0).round().max(1.0)),
    })
}

unsafe fn fetch() {
    unsafe {
        let Ok(vm) = jvm::current_vm() else {
            return;
        };
        let mut args = JavaVMAttachArgs::new(JNI_VERSION_1_8, null(), null_mut());
        let Ok(env) = vm.AttachCurrentThreadAsDaemon(&mut args) else {
            return;
        };
        if env.PushLocalFrame(256) == 0 {
            if let Some((name, uuid)) = session(&env) {
                ACCOUNT.lock().unwrap_or_else(|e| e.into_inner()).name = Some(name);
                let face = skin_url(&env, &uuid).and_then(|url| face(&env, &url));
                ACCOUNT.lock().unwrap_or_else(|e| e.into_inner()).face = face;
            }
            env.ExceptionClear();
            env.PopLocalFrame(null_mut());
        }
        let _ = vm.DetachCurrentThread();
    }
}

unsafe fn ok(env: &JNIEnv, value: jobject) -> Option<jobject> {
    unsafe {
        if env.ExceptionCheck() {
            env.ExceptionClear();
            return None;
        }
        (!value.is_null()).then_some(value)
    }
}

unsafe fn method(env: &JNIEnv, class: jclass, names: &[&str], sigs: &[&str]) -> Option<jmethodID> {
    unsafe {
        for name in names {
            for sig in sigs {
                let id = env.GetMethodID(class, *name, *sig);
                if !id.is_null() {
                    return Some(id);
                }
                env.ExceptionClear();
            }
        }
        None
    }
}

unsafe fn string(env: &JNIEnv, obj: jobject, names: &[&str]) -> Option<String> {
    unsafe {
        let class = env.GetObjectClass(obj);
        let id = method(env, class, names, &["()Ljava/lang/String;"])?;
        let value = ok(env, env.CallObjectMethod0(obj, id))?;
        env.GetStringUTFChars_as_string(value)
    }
}

unsafe fn session(env: &JNIEnv) -> Option<(String, String)> {
    unsafe {
        let reflect = Reflect::new(env).ok()?;
        let (_, class) = classloader::find_game_class_loader(env, &reflect, mappings::MINECRAFT_CLASS_CANDIDATES).ok()?;
        let get = lookup::static_method(env, class, &mappings::GET_MINECRAFT).ok()?;
        let minecraft = ok(env, env.CallStaticObjectMethod0(class, get))?;
        let get_session = method(
            env,
            class,
            &["func_110432_I", "getSession", "L"],
            &["()Lnet/minecraft/util/Session;", "()Lavm;"],
        )?;
        let session = ok(env, env.CallObjectMethod0(minecraft, get_session))?;
        let name = string(env, session, &["func_111285_a", "getUsername", "c"])?;
        let uuid = string(env, session, &["func_148255_b", "getPlayerID", "b"])
            .map(|u| u.replace('-', ""))
            .filter(|u| u.len() == 32 && u.chars().all(|c| c.is_ascii_hexdigit()))
            .unwrap_or_default();
        Some((name, uuid))
    }
}

unsafe fn open(env: &JNIEnv, url: &str) -> Option<jobject> {
    unsafe {
        let url_class = env.FindClass("java/net/URL");
        let init = env.GetMethodID(url_class, "<init>", "(Ljava/lang/String;)V");
        let target = ok(env, env.NewObject1(url_class, init, env.NewStringUTF(url)))?;
        let connect = env.GetMethodID(url_class, "openConnection", "()Ljava/net/URLConnection;");
        let connection = ok(env, env.CallObjectMethod0(target, connect))?;
        let conn_class = env.FindClass("java/net/URLConnection");
        for setter in ["setConnectTimeout", "setReadTimeout"] {
            let id = env.GetMethodID(conn_class, setter, "(I)V");
            env.CallVoidMethod1(connection, id, TIMEOUT_MS);
            ok(env, connection)?;
        }
        let stream = env.GetMethodID(conn_class, "getInputStream", "()Ljava/io/InputStream;");
        ok(env, env.CallObjectMethod0(connection, stream))
    }
}

unsafe fn text(env: &JNIEnv, url: &str) -> Option<String> {
    unsafe {
        let stream = open(env, url)?;
        let scanner_class = env.FindClass("java/util/Scanner");
        let init = env.GetMethodID(scanner_class, "<init>", "(Ljava/io/InputStream;Ljava/lang/String;)V");
        let scanner = ok(env, env.NewObject2(scanner_class, init, stream, env.NewStringUTF("UTF-8")))?;
        let delimiter = env.GetMethodID(scanner_class, "useDelimiter", "(Ljava/lang/String;)Ljava/util/Scanner;");
        ok(env, env.CallObjectMethod1(scanner, delimiter, env.NewStringUTF("\\A")))?;
        let next = env.GetMethodID(scanner_class, "next", "()Ljava/lang/String;");
        let body = ok(env, env.CallObjectMethod0(scanner, next))?;
        let close = env.GetMethodID(scanner_class, "close", "()V");
        env.CallVoidMethod0(scanner, close);
        env.ExceptionClear();
        env.GetStringUTFChars_as_string(body)
    }
}

fn json_string<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let at = json.find(&format!("\"{key}\""))?;
    let rest = &json[at + key.len() + 2..];
    let rest = &rest[rest.find('"')? + 1..];
    Some(&rest[..rest.find('"')?])
}

fn base64(input: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in input.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

unsafe fn skin_url(env: &JNIEnv, uuid: &str) -> Option<String> {
    if uuid.is_empty() {
        return None;
    }
    let profile = unsafe { text(env, &format!("https://sessionserver.mojang.com/session/minecraft/profile/{uuid}"))? };
    let decoded = String::from_utf8(base64(json_string(&profile, "value")?)).ok()?;
    let skin = &decoded[decoded.find("\"SKIN\"")?..];
    let url = json_string(skin, "url")?.replace("\\/", "/");
    let url = url.replacen("http://", "https://", 1);
    url.starts_with("https://textures.minecraft.net/").then_some(url)
}

unsafe fn face(env: &JNIEnv, url: &str) -> Option<Face> {
    unsafe {
        let stream = open(env, url)?;
        let io = env.FindClass("javax/imageio/ImageIO");
        let read = env.GetStaticMethodID(io, "read", "(Ljava/io/InputStream;)Ljava/awt/image/BufferedImage;");
        let image = ok(env, env.CallStaticObjectMethod1(io, read, stream))?;
        let image_class = env.FindClass("java/awt/image/BufferedImage");
        let width = env.GetMethodID(image_class, "getWidth", "()I");
        let height = env.GetMethodID(image_class, "getHeight", "()I");
        let (w, h) = (env.CallIntMethod0(image, width), env.CallIntMethod0(image, height));
        if w < 64 || h < 32 {
            return None;
        }
        let rgb = env.GetMethodID(image_class, "getRGB", "(II)I");
        let pixel = |x: i32, y: i32| env.CallIntMethod2(image, rgb, x * w / 64, y * w / 64) as u32;
        let mut face = [[0u8; 3]; 64];
        for y in 0..8 {
            for x in 0..8 {
                let base = pixel(8 + x, 8 + y);
                let hat = pixel(40 + x, 8 + y);
                let p = if hat >> 24 > 0x7f { hat } else { base };
                face[(y * 8 + x) as usize] = [(p >> 16) as u8, (p >> 8) as u8, p as u8];
            }
        }
        if env.ExceptionCheck() {
            env.ExceptionClear();
            return None;
        }
        Some(face)
    }
}
