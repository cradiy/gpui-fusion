use crate::native::*;
use crate::{MacPlatform, ns_string};
use anyhow::{Context as _, Result, anyhow};
use ctor::ctor;
use gpui::{Action, Menu, MenuItem, TrayId, TrayOptions};
use image::{ExtendedColorType, ImageEncoder as _, codecs::png::PngEncoder};
use objc2::{
    class, msg_send,
    runtime::ClassBuilder as ClassDecl,
    runtime::{AnyClass as Class, AnyObject as Object, Sel},
    sel,
};
use objc2_app_kit::{NSControlStateValueOn, NSEventType};
use objc2_foundation::{NSInteger, NSUInteger};
use std::{
    ffi::{CStr, c_void},
    ptr,
};

const PLATFORM_IVAR: &CStr = c"platform";
const TRAY_ID_IVAR: &CStr = c"trayId";
static mut TRAY_TARGET_CLASS: *const Class = ptr::null();

#[ctor(unsafe)]
unsafe fn build_tray_target_class() {
    unsafe {
        let mut decl = ClassDecl::new(c"GPUITrayTarget", class!(NSObject)).unwrap();
        decl.add_ivar::<*mut c_void>(PLATFORM_IVAR);
        decl.add_ivar::<u32>(TRAY_ID_IVAR);
        decl.add_method(
            sel!(handleGPUITrayActivate:),
            handle_tray_activate as extern "C" fn(*mut Object, Sel, id),
        );
        decl.add_method(
            sel!(handleGPUITrayMenuItem:),
            handle_tray_menu_item as extern "C" fn(*mut Object, Sel, id),
        );
        TRAY_TARGET_CLASS = decl.register();
    }
}

pub(crate) struct MacTray {
    status_item: id,
    target: id,
    menu: id,
    actions: Vec<Box<dyn Action>>,
    activate: Option<Box<dyn Action>>,
}

impl MacTray {
    pub(crate) fn new(
        platform: *const MacPlatform,
        id: TrayId,
        options: TrayOptions,
    ) -> Result<Self> {
        unsafe {
            let image = create_image(&options)?;
            let status_bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
            let status_item: id = msg_send![status_bar, statusItemWithLength: -1.0];
            if status_item.is_null() {
                let _: () = msg_send![image, release];
                anyhow::bail!("creating NSStatusItem failed");
            }

            let target: id = msg_send![TRAY_TARGET_CLASS, new];
            (*target).write_ivar(PLATFORM_IVAR, platform as *mut c_void);
            (*target).write_ivar(TRAY_ID_IVAR, id.as_u32());

            let button: id = msg_send![status_item, button];
            let _: () = msg_send![button, setImage: image];
            let _: () = msg_send![image, release];
            if let Some(tooltip) = &options.tooltip {
                let _: () = msg_send![button, setToolTip: ns_string(tooltip)];
            }
            let _: () = msg_send![button, setTarget: target];
            let _: () = msg_send![button, setAction: sel!(handleGPUITrayActivate:)];
            let mouse_up_mask: NSUInteger = (1 << 2) | (1 << 4);
            let _: NSUInteger = msg_send![button, sendActionOn: mouse_up_mask];

            let mut actions = Vec::new();
            let menu = create_menu(&options.menu, target, &mut actions);
            Ok(Self {
                status_item,
                target,
                menu,
                actions,
                activate: options.activate,
            })
        }
    }

    pub(crate) fn activation_action(&self) -> Option<Box<dyn Action>> {
        self.activate.as_ref().map(|action| action.boxed_clone())
    }

    pub(crate) fn menu_action(&self, index: usize) -> Option<Box<dyn Action>> {
        self.actions.get(index).map(|action| action.boxed_clone())
    }

    pub(crate) fn menu_handles(&self) -> (id, id) {
        (self.status_item, self.menu)
    }
}

impl Drop for MacTray {
    fn drop(&mut self) {
        unsafe {
            let status_bar: id = msg_send![class!(NSStatusBar), systemStatusBar];
            let _: () = msg_send![status_bar, removeStatusItem: self.status_item];
            let _: () = msg_send![self.menu, release];
            let _: () = msg_send![self.target, release];
        }
    }
}

unsafe fn create_image(options: &TrayOptions) -> Result<id> {
    let image = options
        .icon
        .images()
        .iter()
        .max_by_key(|image| image.width.saturating_mul(image.height))
        .ok_or_else(|| anyhow!("tray icon has no image representations"))?;
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            &image.rgba,
            image.width,
            image.height,
            ExtendedColorType::Rgba8,
        )
        .context("encoding tray icon")?;
    unsafe {
        let data: id = msg_send![class!(NSData), dataWithBytes: png.as_ptr().cast::<c_void>(), length: png.len() as NSUInteger];
        let native_image: id = msg_send![class!(NSImage), alloc];
        let native_image: id = msg_send![native_image, initWithData: data];
        anyhow::ensure!(
            !native_image.is_null(),
            "decoding tray icon as NSImage failed"
        );
        let _: () =
            msg_send![native_image, setTemplate: if options.icon.is_template() { YES } else { NO }];
        Ok(native_image)
    }
}

unsafe fn create_menu(items: &[MenuItem], target: id, actions: &mut Vec<Box<dyn Action>>) -> id {
    unsafe {
        let menu: id = msg_send![class!(NSMenu), new];
        for item in items {
            match item {
                MenuItem::Separator => {
                    let value: () = msg_send![menu, addItem: ({ let value: id = msg_send![class!(NSMenuItem), separatorItem]; value })];
                    value
                }
                MenuItem::Label(label) => {
                    let item = ({
                        let value: id = msg_send![class!(NSMenuItem), new];
                        value
                    })
                    .autorelease();
                    let _: () = msg_send![item, setTitle: ns_string(label)];
                    let _: () = msg_send![item, setEnabled: NO];
                    let _: () = msg_send![menu, addItem: item];
                }
                MenuItem::Action {
                    name,
                    action,
                    checked,
                    disabled,
                    ..
                } => {
                    let item: id = msg_send![class!(NSMenuItem), alloc];
                    let item: id = msg_send![item,
                        initWithTitle: ns_string(name)
                        , action: sel!(handleGPUITrayMenuItem:)
                        , keyEquivalent: ns_string("")
                    ];
                    let _: () = msg_send![item, setTarget: target];
                    let tag = actions.len() as NSInteger;
                    let _: () = msg_send![item, setTag: tag];
                    let _: () = msg_send![item, setEnabled: if *disabled { NO } else { YES }];
                    if *checked {
                        let _: () = msg_send![item, setState: NSControlStateValueOn];
                    }
                    actions.push(action.boxed_clone());
                    let _: () = msg_send![menu, addItem: item.autorelease()];
                }
                MenuItem::Submenu(Menu {
                    name,
                    items,
                    disabled,
                }) => {
                    let item = ({
                        let value: id = msg_send![class!(NSMenuItem), new];
                        value
                    })
                    .autorelease();
                    let _: () = msg_send![item, setTitle: ns_string(name)];
                    let _: () = msg_send![item, setEnabled: if *disabled { NO } else { YES }];
                    let submenu = create_menu(items, target, actions).autorelease();
                    let _: () = msg_send![item, setSubmenu: submenu];
                    let _: () = msg_send![menu, addItem: item];
                }
                MenuItem::SystemMenu(_) => {}
            }
        }
        menu
    }
}

extern "C" fn handle_tray_activate(this: *mut Object, _: Sel, _: id) {
    let this = unsafe { &mut *this };
    unsafe {
        let platform = platform_from_target(this);
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let event: id = msg_send![app, currentEvent];
        let event_type: NSEventType = msg_send![event, type];
        platform.handle_tray_activation(
            tray_id_from_target(this),
            event_type == NSEventType::RightMouseUp,
        );
    }
}

extern "C" fn handle_tray_menu_item(this: *mut Object, _: Sel, item: id) {
    let this = unsafe { &mut *this };
    unsafe {
        let tag: NSInteger = msg_send![item, tag];
        platform_from_target(this).handle_tray_menu_action(tray_id_from_target(this), tag as usize);
    }
}

unsafe fn platform_from_target(target: &Object) -> &MacPlatform {
    unsafe {
        let pointer: *mut c_void = *target.read_ivar(PLATFORM_IVAR);
        assert!(!pointer.is_null());
        &*(pointer as *const MacPlatform)
    }
}

unsafe fn tray_id_from_target(target: &Object) -> TrayId {
    unsafe { TrayId::from_u32(*target.read_ivar::<u32>(TRAY_ID_IVAR)) }
}
