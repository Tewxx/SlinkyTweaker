pub mod classloader;
pub mod exception;
pub mod lookup;
pub mod reflect;
pub mod strings;

pub use reflect::Reflect;

pub struct Method {
    pub what: &'static str,
    pub names: &'static [&'static str],
    pub sigs: &'static [&'static str],
}

use crate::error::{Error, Result};
use jni_simple::{JNIEnv, JNI_GetCreatedJavaVMs_first, JavaVM, JavaVMAttachArgs, JNI_VERSION_1_8};
use std::ptr::{null, null_mut};

pub unsafe fn current_vm() -> Result<JavaVM> {
    unsafe {
        match JNI_GetCreatedJavaVMs_first() {
            Ok(Some(vm)) => Ok(vm),
            Ok(None) => Err(Error::Jvm("No JVM is running in this process.".into())),
            Err(e) => Err(Error::Jvm(format!("Couldn't look for a JVM: {e}"))),
        }
    }
}

pub unsafe fn attach_current_thread(vm: &JavaVM) -> Result<JNIEnv> {
    unsafe {
        let mut args = JavaVMAttachArgs::new(JNI_VERSION_1_8, null(), null_mut());
        vm.AttachCurrentThread(&mut args)
            .map_err(|e| Error::Jvm(format!("Couldn't attach this thread to the JVM: {e}")))
    }
}
