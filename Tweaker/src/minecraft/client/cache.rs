use super::*;
use std::collections::HashMap;
#[derive(Default)]
pub(crate) struct Cache {
    classes: HashMap<&'static str, jclass>,
    fields: HashMap<&'static str, jfieldID>,
    methods: HashMap<&'static str, jmethodID>,
}
impl Cache {
    pub(crate) unsafe fn object_method(
        &mut self,
        c: &Client,
        obj: jobject,
        key: &'static str,
        names: &[&str],
        sigs: &[&str],
    ) -> Result<jmethodID> {
        if let Some(&v) = self.methods.get(key) {
            return Ok(v);
        }
        let cls = unsafe { c.env.GetObjectClass(obj) };
        let result = unsafe { self.method(c, cls, key, names, sigs, false) };
        unsafe { c.env.DeleteLocalRef(cls) };
        result
    }
    pub(crate) unsafe fn player(&mut self, c: &Client) -> Result<jobject> {
        let field = unsafe {
            self.field(
                c,
                c.instance,
                "player",
                &["field_71439_g", "thePlayer", "h"],
                &["Lnet/minecraft/client/entity/EntityPlayerSP;", "Lbew;"],
            )?
        };
        Ok(unsafe { c.env.GetObjectField(c.instance, field) })
    }
    pub(crate) unsafe fn class(
        &mut self,
        c: &Client,
        key: &'static str,
        names: &[&str],
    ) -> Result<jclass> {
        if let Some(&v) = self.classes.get(key) {
            return Ok(v);
        }
        let local = unsafe { load_first_class(&c.env, c.loader, names, key)? };
        let v = unsafe { c.env.NewGlobalRef(local) };
        if v.is_null() {
            return Err(Error::Jvm(format!("{key}: global reference failed")));
        }
        self.classes.insert(key, v);
        Ok(v)
    }
    pub(crate) unsafe fn field(
        &mut self,
        c: &Client,
        obj: jobject,
        key: &'static str,
        names: &[&str],
        sigs: &[&str],
    ) -> Result<jfieldID> {
        if let Some(&v) = self.fields.get(key) {
            return Ok(v);
        }
        let cls = unsafe { c.env.GetObjectClass(obj) };
        let v = unsafe { find_field(&c.env, cls, names, sigs, key)? };
        unsafe { c.env.DeleteLocalRef(cls) };
        self.fields.insert(key, v);
        Ok(v)
    }
    pub(crate) unsafe fn method(
        &mut self,
        c: &Client,
        cls: jclass,
        key: &'static str,
        names: &[&str],
        sigs: &[&str],
        is_static: bool,
    ) -> Result<jmethodID> {
        if let Some(&v) = self.methods.get(key) {
            return Ok(v);
        }
        let v = unsafe {
            if is_static {
                find_static_method(&c.env, cls, names, sigs, key)?
            } else {
                find_method(&c.env, cls, names, sigs, key)?
            }
        };
        self.methods.insert(key, v);
        Ok(v)
    }
}
