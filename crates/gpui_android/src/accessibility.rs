use crate::bridge::{call, window};
use accesskit::{ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
use accesskit_android::{Adapter, PlatformAction};
use gpui::A11yCallbacks;
use jni::{
    JNIEnv,
    objects::{JClass, JObject},
    sys::{jboolean, jfloat, jint, jlong, jobject},
};

pub(crate) struct Accessibility {
    pub adapter: Adapter,
    callbacks: A11yCallbacks,
}

impl Accessibility {
    pub fn new(callbacks: A11yCallbacks) -> Self {
        Self {
            adapter: Adapter::default(),
            callbacks,
        }
    }

    fn reset(&mut self) {
        self.adapter = Adapter::default();
        (self.callbacks.deactivation)();
    }
}

struct Activation<'a>(&'a A11yCallbacks);

impl ActivationHandler for Activation<'_> {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        (self.0.activation)()
    }
}

struct Actions<'a> {
    callbacks: &'a A11yCallbacks,
    handled: bool,
}

impl ActionHandler for Actions<'_> {
    fn do_action(&mut self, request: ActionRequest) {
        self.handled = true;
        (self.callbacks.action)(request);
    }
}

pub(crate) extern "system" fn node(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    host: JObject,
    node: jint,
    focus: jboolean,
) -> jobject {
    call(&mut env, |env| {
        let window = window(id)?;
        let mut state = window.accessibility.borrow_mut();
        let Some(Accessibility { adapter, callbacks }) = state.as_mut() else {
            return Ok(std::ptr::null_mut());
        };
        let mut activation = Activation(callbacks);
        let info = if focus != 0 {
            adapter.find_focus(&mut activation, env, &host, node)
        } else {
            adapter.create_accessibility_node_info(&mut activation, env, &host, node)
        };
        Ok(info.into_raw())
    })
}

pub(crate) extern "system" fn action(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    host: JObject,
    node: jint,
    action: jint,
    arguments: JObject,
) -> jboolean {
    call(&mut env, |env| {
        let Some(action) = PlatformAction::from_java(env, action, &arguments) else {
            return Ok(0);
        };
        let window = window(id)?;
        let (events, handled) = {
            let mut state = window.accessibility.borrow_mut();
            let Some(Accessibility { adapter, callbacks }) = state.as_mut() else {
                return Ok(0);
            };
            let mut actions = Actions {
                callbacks,
                handled: false,
            };
            let events = adapter.perform_action(&mut actions, node, &action);
            (events, actions.handled)
        };
        if let Some(events) = events {
            events.raise(env, &host);
            Ok(1)
        } else {
            Ok(handled.into())
        }
    })
}

pub(crate) extern "system" fn hover(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    host: JObject,
    action: jint,
    x: jfloat,
    y: jfloat,
) -> jboolean {
    call(&mut env, |env| {
        let window = window(id)?;
        let events = {
            let mut state = window.accessibility.borrow_mut();
            let Some(Accessibility { adapter, callbacks }) = state.as_mut() else {
                return Ok(0);
            };
            adapter.on_hover_event(&mut Activation(callbacks), action, x, y)
        };
        if let Some(events) = events {
            events.raise(env, &host);
            Ok(1)
        } else {
            Ok(0)
        }
    })
}

pub(crate) extern "system" fn reset(mut env: JNIEnv, _: JClass, id: jlong) {
    call(&mut env, |_| {
        if let Some(state) = window(id)?.accessibility.borrow_mut().as_mut() {
            state.reset();
        }
        Ok(())
    });
}
