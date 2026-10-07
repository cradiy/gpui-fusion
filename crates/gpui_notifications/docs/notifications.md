# System notifications

## Create a notification center

Use the installed GPUI platform to create a center. Retain it for as long as the
application needs to receive responses. Desktop and browser applications may
also use `NotificationCenter::new`; Android requires the platform host.

```rust
use gpui::gpui_notifications::{Notification, NotificationOptions};

let request = cx.notifications(NotificationOptions::new(
    "com.example.editor",
    "Example Editor",
));
// Await in a foreground task, then store the center in application state.
let center = request.await?;
let events = center.take_events().expect("one event consumer");
```

Create the center at application startup so notification clicks can be handled
when the operating system launches the application. Clones share one event
stream. Use one center per application on desktop and Web; Android supports
one center per channel in a host session.

Check `center.permission().await?` before posting. Call
`center.request_permission()` directly in a user gesture handler, then await
the returned future in a task. Permission prompts belong to Android, macOS,
and browsers. Linux has no standard prompt; Windows exposes the user's system
setting. A granted result does not override Do Not Disturb or delivery policies.

## Send, update, and remove

```rust
let mut notification = Notification::new(
    "export",
    "Export complete",
    "Your document is ready.",
);
notification.silent = true;
center.show(notification).await?;
center.remove("export").await?;
```

Reusing an ID updates the existing notification. IDs use 1–64 ASCII letters,
digits, dots, underscores, or hyphens. Titles and bodies are plain text.
Progress is optional and ranges from 0 to 100. Set it only when
`center.capabilities().progress` is true. Linux progress uses the desktop's
`value` hint and may be ignored by its notification server.

Dropping the center releases callbacks but does not withdraw delivered
notifications. Call `remove` explicitly when a notification becomes irrelevant.

## Custom icons

Pass an optional icon with each notification:

```rust
use gpui::gpui_notifications::NotificationIcon;

notification.icon = Some(NotificationIcon::Resource("ic_message".into()));
// Or, on platforms supporting image icons:
notification.icon = Some(NotificationIcon::ImageUri(
    "file:///absolute/path/message.png".into(),
));
```

| Platform | Resource | Image URI |
| --- | --- | --- |
| Android | Application drawable name | Falls back to application icon |
| Linux / FreeBSD | Icon theme name | Local file URI |
| Windows | Falls back to application icon | Local file URI |
| macOS | System application icon | System application icon |
| Web | Browser default | HTTPS image URL, subject to browser policy |

`None` and unsupported source kinds use the platform's default icon behavior.
The framework does not download or decode icon images. An unreadable image URI
is handled by the native notification service. Android also falls back when a
drawable name cannot be resolved: it uses the configured notification icon,
application icon, or Android's generic application icon. Use a monochrome drawable for Android
small icons. The OS may still show the application icon prominently in its
notification header; a custom small icon does not replace application identity.

## Actions and replies

```rust
use gpui::gpui_notifications::{NotificationAction, NotificationEvent};

let capabilities = center.capabilities();
if capabilities.inline_reply {
    notification.actions.push(
        NotificationAction::new("reply", "Reply").reply("Your message"),
    );
}
center.show(notification).await?;

while let Ok(event) = events.recv().await {
    match event {
        NotificationEvent::Activated { id, action, reply } => {
            // Dispatch to application logic. Body clicks have action == None.
        }
        NotificationEvent::Dismissed { id } => {}
        NotificationEvent::Failed { id, message } => {}
    }
}
```

Android, Windows, and macOS support native text replies. Linux and Web expose
ordinary buttons where available. Check `max_actions` before building actions;
unsupported replies, excessive buttons, and unsupported progress return errors
rather than silently removing behavior. `default` and `dismiss` are reserved
action IDs. The operating system decides how many buttons are visible at once.

Dismissal callbacks are best effort and may include expiry as well as manual
closure. They are not delivery receipts. Do not depend on receiving a dismissal
after the application exits. The framework returns reply text to the application;
it does not send messages or execute business actions.

## Platform setup

### Android

The `app_id` must match `platforms.android.application-id`. Declare
`android.permission.POST_NOTIFICATIONS` in `gpuiforge.json`. Set
`options.channel.id` and `options.channel.name` before creating the center.
Channel importance and user overrides remain under Android's control.

Enable the host module and optionally configure a default small icon:

```json
{
  "platforms": {
    "android": {
      "application-id": "com.example.app",
      "features": ["notifications"],
      "notification-icon": "assets/notification.xml",
      "permissions": ["android.permission.POST_NOTIFICATIONS"]
    }
  }
}
```

Run `gpuiforge sync` to regenerate the managed project. The default icon needs
no per-send `Resource`; applications can still override it with another packaged
drawable name. Generated hosts register the notification receiver and forward
Activity intents. A custom host must forward notification intents with
`NotificationStore.receive` as `GpuiActivity` does.

Responses received while the Rust host is absent are stored privately and
drained when the matching channel connects. This is response handoff, not a
persistent work queue. Notifications do not keep the process alive.

### Linux and FreeBSD

A session bus and `org.freedesktop.Notifications` server are required. Use the
desktop entry ID as `app_id`. Available buttons follow the server's advertised
capabilities. The connection receives actions while the application is running;
this backend does not implement notification activation after process exit or
the sandbox portal notification API.

### Windows

Use the application's AUMID as `app_id`. For an unpackaged application, set
`options.windows_register_application = true` to register the current executable
under the current user's `Software\Classes` registry keys. Registration persists
and must be removed by the application's uninstaller.

The activator CLSID is UUID v5 with the URL namespace and the UTF-8 name
`gpui-notification:<app_id>`. The class's `LocalServer32` contains the quoted
executable path. The AUMID key stores `DisplayName` and `CustomActivator`.
Installers may supply these entries instead of enabling runtime registration.
Packaged applications must declare the corresponding COM activator in their
manifest. See Microsoft's [desktop activation guide](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl).

Create the center on the GPUI application thread, which initializes COM and
pumps Windows messages. Keep it alive to receive both existing-process and
system-launched activation. Standalone callers must initialize COM themselves.
Applications parsing command-line arguments must accept COM's `-Embedding`
launch argument and initialize the notification center on that path as well.

### macOS

Run inside an application bundle whose bundle identifier matches `app_id`.
The center owns `UNUserNotificationCenter`'s delegate; reuse it rather than
installing a second delegate. Create it during application startup to receive
activation. The small application icon remains controlled by macOS.

### Web

Use HTTPS or localhost. For ordinary page notifications, no worker is required.
For persistent notifications and action buttons, serve
`assets/notification-worker.js` from the application origin and import it from
your service worker. Set `options.web_service_worker` to that worker's URL.
The worker's scope must cover the application page.

Clicks focus the originating URL or reopen it. Responses wait in origin-local
storage until a center connects. Browsers without a service worker support only
page-lifetime notification callbacks. Button counts, icon appearance, and media
surfaces depend on the browser. This API does not subscribe to remote push.
