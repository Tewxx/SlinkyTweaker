use super::{cache::Cache, *};
mod input;
mod place;
mod timer;

pub struct Game {
    pub(super) client: Client,
    pub(super) cache: Cache,
}
impl Game {
    pub fn attach() -> Result<Self> {
        Ok(Self {
            client: Client::attach()?,
            cache: Cache::default(),
        })
    }

    pub fn scope<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        unsafe {
            if self.client.env.PushLocalFrame(128) < 0 {
                return Err(Error::Jvm("Game local frame failed".into()));
            }
            struct LocalFrame(JNIEnv);
            impl Drop for LocalFrame {
                fn drop(&mut self) {
                    unsafe {
                        self.0.PopLocalFrame(std::ptr::null_mut());
                    }
                }
            }
            let _frame = LocalFrame(self.client.env);
            let result = f(self);
            exception::check(&self.client.env, "Game callback")?;
            result
        }
    }

    unsafe fn object(
        &mut self,
        obj: jobject,
        key: &'static str,
        names: &[&str],
        sigs: &[&str],
    ) -> Result<jobject> {
        unsafe {
            let field = self.cache.field(&self.client, obj, key, names, sigs)?;
            Ok(self.client.env.GetObjectField(obj, field))
        }
    }

    pub fn delete_global(&self, obj: jobject) {
        unsafe { self.client.env.DeleteGlobalRef(obj) }
    }
    pub fn clear_exception(&self) {
        unsafe { self.client.env.ExceptionClear() }
    }
}
