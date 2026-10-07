use super::*;
use ::windows::{
    Win32::{
        Foundation::{CLASS_E_NOAGGREGATION, E_INVALIDARG, E_POINTER},
        System::Com::{
            CLSCTX_LOCAL_SERVER, CoRegisterClassObject, CoRevokeClassObject, IClassFactory,
            IClassFactory_Impl, REGCLS_MULTIPLEUSE,
        },
        UI::Notifications::{
            INotificationActivationCallback, INotificationActivationCallback_Impl,
            NOTIFICATION_USER_INPUT_DATA,
        },
    },
    core::{BOOL, GUID, IUnknown, Interface, PCWSTR, implement},
};
use std::ffi::c_void;

#[implement(INotificationActivationCallback)]
struct Callback {
    app_id: String,
    events: async_channel::Sender<NotificationEvent>,
}
#[allow(non_snake_case)]
impl INotificationActivationCallback_Impl for Callback_Impl {
    fn Activate(
        &self,
        app: &PCWSTR,
        args: &PCWSTR,
        input: *const NOTIFICATION_USER_INPUT_DATA,
        count: u32,
    ) -> ::windows::core::Result<()> {
        if app.is_null() || args.is_null() || (input.is_null() && count != 0) {
            return Err(E_POINTER.into());
        }
        let app = unsafe { app.to_string()? };
        if app != self.app_id {
            return Err(E_INVALIDARG.into());
        }
        let args = unsafe { args.to_string()? };
        let Some((id, action)) = args.split_once(':') else {
            return Err(E_INVALIDARG.into());
        };
        if validate_id(id).is_err() || validate_id(action).is_err() {
            return Err(E_INVALIDARG.into());
        }
        let mut reply = None;
        if count > 0 {
            for field in unsafe { std::slice::from_raw_parts(input, count as usize) } {
                if !field.Key.is_null()
                    && !field.Value.is_null()
                    && unsafe { field.Key.to_string()? } == action
                {
                    reply = Some(unsafe { field.Value.to_string()? });
                }
            }
        }
        let _ = self.events.try_send(NotificationEvent::Activated {
            id: id.into(),
            action: (action != "default").then(|| action.into()),
            reply,
        });
        Ok(())
    }
}
#[implement(IClassFactory)]
struct Factory {
    callback: INotificationActivationCallback,
}
#[allow(non_snake_case)]
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: ::windows::core::Ref<'_, IUnknown>,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> ::windows::core::Result<()> {
        if out.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            *out = std::ptr::null_mut();
        }
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        unsafe { self.callback.query(iid, out).ok() }
    }
    fn LockServer(&self, _: BOOL) -> ::windows::core::Result<()> {
        Ok(())
    }
}

pub(super) struct Registration(u32);
impl Registration {
    pub fn new(
        options: &NotificationOptions,
        events: async_channel::Sender<NotificationEvent>,
    ) -> Result<Self> {
        let id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("gpui-notification:{}", options.app_id).as_bytes(),
        );
        let clsid = GUID::from_u128(id.as_u128());
        if options.windows_register_application {
            ensure!(
                !options.app_id.contains(['\\', '/']) && options.app_id.len() <= 128,
                "invalid Windows application identity"
            );
            let executable = std::env::current_exe()?;
            let executable = executable
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Windows executable path is not Unicode"))?;
            let class = format!("{{{id}}}");
            let server = windows_registry::CURRENT_USER
                .create(format!("Software\\Classes\\CLSID\\{class}\\LocalServer32"))?;
            server.set_string("", format!("\"{executable}\""))?;
            let app = windows_registry::CURRENT_USER.create(format!(
                "Software\\Classes\\AppUserModelId\\{}",
                options.app_id
            ))?;
            app.set_string("DisplayName", &options.app_name)?;
            app.set_string("CustomActivator", class)?;
        }
        let callback: INotificationActivationCallback = Callback {
            app_id: options.app_id.clone(),
            events,
        }
        .into();
        let factory: IClassFactory = Factory { callback }.into();
        Ok(Self(unsafe {
            CoRegisterClassObject(&clsid, &factory, CLSCTX_LOCAL_SERVER, REGCLS_MULTIPLEUSE)?
        }))
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        let _ = unsafe { CoRevokeClassObject(self.0) };
    }
}
