// Import this script from the application's same-origin service worker.
self.addEventListener("notificationclick", event => {
    const data = event.notification.data;
    if (!data?.gpuiNotifications) return;
    event.notification.close();
    event.waitUntil((async () => {
        const url = new URL(data.url);
        if (url.origin !== self.location.origin) return;
        const clients = await self.clients.matchAll({type: "window", includeUncontrolled: true});
        const client = clients.find(client => client.url === url.href);
        const message = { gpuiNotifications: data.gpuiNotifications, event: {kind: "activated", id: data.id, action: event.action || null, reply: null} };
        const cache = await caches.open("gpui-notification-responses");
        await cache.put(new Request(new URL("__gpui_notification__/" + encodeURIComponent(data.gpuiNotifications) + "/" + data.id, self.registration.scope)), new Response(JSON.stringify(message)));
        if (client) { await client.focus(); client.postMessage({gpuiNotifications: data.gpuiNotifications}); }
        else await self.clients.openWindow(url.href);
    })());
});
self.addEventListener("notificationclose", event => {
    const data = event.notification.data;
    if (!data?.gpuiNotifications) return;
    event.waitUntil(self.clients.matchAll({type: "window", includeUncontrolled: true}).then(clients => {
        for (const client of clients) client.postMessage({gpuiNotifications: data.gpuiNotifications, event: {kind: "dismissed", id: data.id}});
    }));
});
