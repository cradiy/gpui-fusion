pub(crate) use core_graphics::base::CGFloat;
use std::ffi::CStr;

use objc2::{Encode, msg_send, runtime::AnyObject};

pub(crate) unsafe fn ns_array(objects: &[id]) -> id {
    unsafe {
        msg_send![objc2::class!(NSArray), arrayWithObjects: objects.as_ptr(), count: objects.len()]
    }
}

#[allow(non_camel_case_types)]
pub(crate) type id = *mut AnyObject;
#[allow(non_upper_case_globals)]
pub(crate) const nil: id = std::ptr::null_mut();
pub(crate) use objc2::runtime::Bool as BOOL;
pub(crate) const YES: BOOL = BOOL::YES;
pub(crate) const NO: BOOL = BOOL::NO;

pub(crate) trait NativeObjectExt {
    unsafe fn read_ivar<T: Encode>(&self, name: &CStr) -> &T;
    unsafe fn write_ivar<T: Encode>(&self, name: &CStr, value: T);
}

impl NativeObjectExt for AnyObject {
    unsafe fn read_ivar<T: Encode>(&self, name: &CStr) -> &T {
        unsafe { self.class().instance_variable(name).unwrap().load(self) }
    }

    unsafe fn write_ivar<T: Encode>(&self, name: &CStr, value: T) {
        unsafe {
            self.class()
                .instance_variable(name)
                .unwrap()
                .load_ptr::<T>(self)
                .write(value);
        }
    }
}

pub(crate) trait NativePointerExt {
    unsafe fn autorelease(self) -> id;
}

impl NativePointerExt for id {
    unsafe fn autorelease(self) -> id {
        unsafe { msg_send![self, autorelease] }
    }
}
