# Android background execution

Use `AndroidPlatform::background_execution()` to request a session-bound
foreground service for application-owned uploads and downloads. The API grants
execution time and displays a notification; the application performs the I/O.

Enable the host module and declare its permissions in `gpuiforge.json`:

```json
{
  "platforms": {
    "android": {
      "application-id": "dev.example.app",
      "features": ["data-sync"],
      "permissions": [
        "android.permission.FOREGROUND_SERVICE",
        "android.permission.FOREGROUND_SERVICE_DATA_SYNC",
        "android.permission.POST_NOTIFICATIONS"
      ]
    }
  }
}
```

Run `gpuiforge sync` to generate the service. `notification-icon` configures its
default small icon; a notification's `icon` field can select another drawable.
Omitting `data-sync` omits its service and Kotlin implementation.

Start execution from a user action while the Activity is active. Obtain the
handle during application startup with
`gpui_android::current_platform().background_execution()`, store it in application
state, and clone it into a GPUI foreground task:

```rust,no_run
# async fn example(execution: gpui_android::AndroidBackgroundExecution) -> anyhow::Result<()> {
let mut lease = execution.start_data_sync(gpui_android::DataSyncNotification {
    channel: "transfers".into(),
    channel_name: "File transfers".into(),
    title: "Uploading files".into(),
    body: "Preparing transfer".into(),
    icon: None,
}).await?;

// Run application-owned I/O on a background executor while retaining `lease`.
// Race the work against `lease.stopped()` and cancel it if Android stops the service.
// Call `lease.update(notification)` to change notification content.
drop(lease); // Stop foreground execution when work finishes or is cancelled.
# Ok(())
# }
```

Only one lease may be active or stopping per process. Awaiting start confirms
that Android accepted foreground promotion. Dropping a pending start cancels
it; dropping the returned lease stops the service and removes its notification.
Notification updates must retain the original channel and channel name.

`POST_NOTIFICATIONS` is a runtime permission on Android 13 and later. Request it
through `AndroidPermissions` to show notifications in the drawer. Android can
run a foreground service when notification permission is denied, but displays
it only in the system's active-apps interface. The other two permissions are
manifest declarations and do not show a dialog.

Android enforces foreground-service start restrictions and data-sync time
limits. Start failures return errors. On a live-process timeout, GPUI stops the
service promptly and resolves `lease.stopped()` with `Timeout`; the application
must cancel its own transfer. See Android's
[foreground service timeouts](https://developer.android.com/develop/background-work/services/fgs/timeout).

Home, backgrounding and retained Activity configuration changes preserve the
session and its lease. Closing the session, including finishing its Activity,
stops execution. A foreground service does not guarantee survival after process
termination, network access during all power-saving modes, or task restoration.
There is no automatic restart, boot receiver, wake lock or persistent scheduler.
Applications own transfer queues, credentials, cancellation and recovery state.
