use super::*;
use block2::{DynBlock, RcBlock};
use objc2::{
    DefinedClass, define_class, msg_send,
    rc::Retained,
    runtime::{Bool, ProtocolObject},
};
use objc2_foundation::{NSArray, NSBundle, NSError, NSObject, NSObjectProtocol, NSSet, NSString};
use objc2_user_notifications::*;
use std::{collections::HashMap, ptr::NonNull, sync::Mutex};

struct DelegateState {
    events: async_channel::Sender<NotificationEvent>,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "GpuiNotificationDelegate"]
    #[ivars = DelegateState]
    struct Delegate;
    unsafe impl NSObjectProtocol for Delegate {}
    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn present(
            &self,
            _: &UNUserNotificationCenter,
            notification: &UNNotification,
            done: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            let mut options =
                UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List;
            if notification.request().content().sound().is_some() {
                options |= UNNotificationPresentationOptions::Sound;
            }
            done.call((options,));
        }
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn respond(
            &self,
            _: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            done: &DynBlock<dyn Fn()>,
        ) {
            let id = response.notification().request().identifier().to_string();
            let action = response.actionIdentifier();
            let event = if *action == *unsafe { UNNotificationDismissActionIdentifier } {
                NotificationEvent::Dismissed { id }
            } else {
                let reply = response
                    .downcast_ref::<UNTextInputNotificationResponse>()
                    .map(|r| r.userText().to_string());
                NotificationEvent::Activated {
                    id,
                    action: (*action != *unsafe { UNNotificationDefaultActionIdentifier })
                        .then(|| action.to_string()),
                    reply,
                }
            };
            let _ = self.ivars().events.try_send(event);
            done.call(());
        }
    }
);

struct MacNotifications {
    center: Retained<UNUserNotificationCenter>,
    _delegate: Retained<Delegate>,
    categories: RefCell<HashMap<String, Retained<UNNotificationCategory>>>,
}

pub async fn create(options: NotificationOptions) -> Result<NotificationCenter> {
    use objc2::AnyThread;
    let bundle = NSBundle::mainBundle().bundleIdentifier();
    ensure!(
        bundle
            .as_ref()
            .is_some_and(|id| id.to_string() == options.app_id),
        "macOS notifications require a bundled application with matching bundle ID"
    );
    let center = UNUserNotificationCenter::currentNotificationCenter();
    ensure!(
        center.delegate().is_none(),
        "a notification center is already registered; reuse its handle"
    );
    let (events, receiver) = async_channel::unbounded();
    let allocated = Delegate::alloc().set_ivars(DelegateState { events });
    let delegate: Retained<Delegate> = unsafe { msg_send![super(allocated), init] };
    center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    Ok(NotificationCenter::from_backend(
        Rc::new(MacNotifications {
            center,
            _delegate: delegate,
            categories: RefCell::default(),
        }),
        receiver,
    ))
}

impl NotificationBackend for MacNotifications {
    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            max_actions: 4,
            inline_reply: true,
            dismissal_events: true,
            progress: false,
            resource_icons: false,
            image_icons: false,
        }
    }
    fn permission(&self, request: bool) -> LocalBoxFuture<'static, Result<NotificationPermission>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        let tx = Mutex::new(Some(tx));
        if request {
            self.center
                .requestAuthorizationWithOptions_completionHandler(
                    UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                    &RcBlock::new(move |granted: Bool, error: *mut NSError| {
                        let result = if let Some(error) = unsafe { error.as_ref() } {
                            Err(anyhow::anyhow!(error.to_string()))
                        } else {
                            Ok(if granted.as_bool() {
                                NotificationPermission::Granted
                            } else {
                                NotificationPermission::Denied
                            })
                        };
                        if let Some(tx) = tx.lock().unwrap().take() {
                            let _ = tx.send(result);
                        }
                    }),
                );
        } else {
            self.center
                .getNotificationSettingsWithCompletionHandler(&RcBlock::new(
                    move |settings: NonNull<UNNotificationSettings>| {
                        let status = unsafe { settings.as_ref() }.authorizationStatus();
                        let permission = if status == UNAuthorizationStatus::NotDetermined {
                            NotificationPermission::NotDetermined
                        } else if status == UNAuthorizationStatus::Denied {
                            NotificationPermission::Denied
                        } else {
                            NotificationPermission::Granted
                        };
                        if let Some(tx) = tx.lock().unwrap().take() {
                            let _ = tx.send(Ok(permission));
                        }
                    },
                ));
        }
        Box::pin(async move { rx.await? })
    }
    fn show(&self, notification: Notification) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            ensure!(
                self.permission(false).await? == NotificationPermission::Granted,
                "notification permission has not been granted"
            );
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&notification.title));
            content.setBody(&NSString::from_str(&notification.body));
            if !notification.silent {
                content.setSound(Some(&UNNotificationSound::defaultSound()));
            }
            let actions: Vec<Retained<UNNotificationAction>> = notification.actions.iter().map(|a| {
                let id = NSString::from_str(&a.id); let title = NSString::from_str(&a.label);
                if let Some(placeholder) = &a.reply_placeholder {
                    UNTextInputNotificationAction::actionWithIdentifier_title_options_textInputButtonTitle_textInputPlaceholder(
                        &id, &title, UNNotificationActionOptions::empty(), &title, &NSString::from_str(placeholder),
                    ).into_super()
                } else { UNNotificationAction::actionWithIdentifier_title_options(&id, &title, UNNotificationActionOptions::empty()) }
            }).collect();
            let category_id = NSString::from_str(&format!("gpui.{}", notification.id));
            let category =
                UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                    &category_id,
                    &NSArray::from_retained_slice(&actions),
                    &NSArray::new(),
                    UNNotificationCategoryOptions::CustomDismissAction,
                );
            {
                let mut categories = self.categories.borrow_mut();
                categories.insert(notification.id.clone(), category);
                self.center
                    .setNotificationCategories(&NSSet::from_retained_slice(
                        &categories.values().cloned().collect::<Vec<_>>(),
                    ));
            }
            content.setCategoryIdentifier(&category_id);
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &NSString::from_str(&notification.id),
                &content,
                None,
            );
            let (tx, rx) = futures::channel::oneshot::channel();
            let tx = Mutex::new(Some(tx));
            self.center.addNotificationRequest_withCompletionHandler(
                &request,
                Some(&RcBlock::new(move |error: *mut NSError| {
                    let result = unsafe { error.as_ref() }
                        .map_or(Ok(()), |error| Err(anyhow::anyhow!(error.to_string())));
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let _ = tx.send(result);
                    }
                })),
            );
            rx.await?
        })
    }
    fn remove(&self, id: String) -> LocalBoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let ids = NSArray::from_retained_slice(&[NSString::from_str(&id)]);
            self.center
                .removePendingNotificationRequestsWithIdentifiers(&ids);
            self.center
                .removeDeliveredNotificationsWithIdentifiers(&ids);
            Ok(())
        })
    }
}
impl Drop for MacNotifications {
    fn drop(&mut self) {
        self.center.setDelegate(None);
    }
}
