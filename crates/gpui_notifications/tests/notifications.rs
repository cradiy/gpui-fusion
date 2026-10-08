use futures::future::LocalBoxFuture;
use gpui_notifications::*;
use std::{cell::RefCell, rc::Rc};

struct Capture(RefCell<Vec<Notification>>);
impl NotificationBackend for Capture {
    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            max_actions: 1,
            inline_reply: false,
            dismissal_events: false,
            progress: false,
            resource_icons: false,
            image_icons: true,
        }
    }
    fn permission(
        &self,
        _: bool,
    ) -> LocalBoxFuture<'static, anyhow::Result<NotificationPermission>> {
        Box::pin(async { Ok(NotificationPermission::Granted) })
    }
    fn show(&self, notification: Notification) -> LocalBoxFuture<'_, anyhow::Result<()>> {
        self.0.borrow_mut().push(notification);
        Box::pin(async { Ok(()) })
    }
    fn remove(&self, _: String) -> LocalBoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn icon_falls_back_without_discarding_content_or_silently_losing_actions() {
    futures::executor::block_on(async {
        let backend = Rc::new(Capture(RefCell::default()));
        let (_, events) = async_channel::unbounded();
        let center = NotificationCenter::from_backend(backend.clone(), events);
        let mut item = Notification::new("message", "Title", "你好 <&>");
        item.icon = Some(NotificationIcon::Resource("chat".into()));
        center.show(item.clone()).await.unwrap();
        assert!(backend.0.borrow()[0].icon.is_none());
        assert_eq!(backend.0.borrow()[0].body, item.body);
        item.icon = Some(NotificationIcon::ImageUri("file:///tmp/chat.png".into()));
        center.show(item.clone()).await.unwrap();
        assert!(matches!(
            backend.0.borrow()[1].icon,
            Some(NotificationIcon::ImageUri(_))
        ));
        item.actions
            .push(NotificationAction::new("reply", "Reply").reply("Message"));
        assert!(center.show(item).await.is_err());
        assert_eq!(backend.0.borrow().len(), 2);
        assert!(center.take_events().is_some());
        assert!(center.clone().take_events().is_none());
    });
}
