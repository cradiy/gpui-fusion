export async function create(app, worker, callback) {
    if (!globalThis.isSecureContext || !globalThis.Notification) throw new Error("Notifications require a secure context and browser support");
    const center = { app, callback, local: new Map(), registration: null, listener: null, draining: Promise.resolve() };
    if (worker) {
        center.registration = await navigator.serviceWorker.register(worker);
        if (!center.registration.active) {
            await new Promise((resolve, reject) => {
                const pending = center.registration.installing || center.registration.waiting;
                if (!pending) return reject(new Error("Notification worker did not activate"));
                const timer = setTimeout(() => { pending.removeEventListener("statechange", changed); reject(new Error("Notification worker activation timed out")); }, 15000);
                function changed() {
                    if (pending.state === "activated" || pending.state === "redundant") {
                        clearTimeout(timer); pending.removeEventListener("statechange", changed);
                        if (pending.state === "activated") resolve(); else reject(new Error("Notification worker failed"));
                    }
                }
                pending.addEventListener("statechange", changed); changed();
            });
        }
        center.listener = event => {
            if (event.source !== center.registration.active || event.data?.gpuiNotifications !== app) return;
            if (event.data.event) center.callback?.(JSON.stringify(event.data.event));
            else drain(center).catch(console.error);
        };
        navigator.serviceWorker.addEventListener("message", center.listener);
        try { await drain(center); }
        catch (error) { close(center); throw error; }
    }
    return center;
}
function drain(center) {
    const read = async () => {
        const cache = await caches.open("gpui-notification-responses");
        const prefix = new URL("__gpui_notification__/" + encodeURIComponent(center.app) + "/", center.registration.scope).href;
        for (const request of await cache.keys()) {
            if (!center.callback || !request.url.startsWith(prefix)) continue;
            const response = await cache.match(request);
            if (!response) continue;
            const message = await response.json();
            await cache.delete(request);
            center.callback?.(JSON.stringify(message.event));
        }
    };
    center.draining = center.draining.catch(() => {}).then(() => navigator.locks
        ? navigator.locks.request("gpui-notifications:" + center.app, read) : read());
    return center.draining;
}
export function permission(request) { return request ? Notification.requestPermission() : Promise.resolve(Notification.permission); }
export function max_actions(center) { return center.registration ? (Notification.maxActions || 0) : 0; }
export async function show(center, json) {
    if (Notification.permission !== "granted") throw new Error("Notification permission has not been granted");
    const item = JSON.parse(json), tag = center.app + ":" + item.id;
    const options = { body: item.body, tag, silent: item.silent, data: { gpuiNotifications: center.app, id: item.id, url: location.href } };
    if (item.icon) options.icon = item.icon.value;
    if (center.registration) {
        options.actions = item.actions.map(a => ({ action: a.id, title: a.label }));
        await center.registration.showNotification(item.title, options);
    } else {
        const previous = center.local.get(item.id);
        if (previous) { detach(previous); previous.close(); }
        const notification = new Notification(item.title, options);
        notification.onclick = () => { window.focus(); center.callback?.(JSON.stringify({kind: "activated", id: item.id, action: null, reply: null})); };
        notification.onclose = () => { center.callback?.(JSON.stringify({kind: "dismissed", id: item.id})); center.local.delete(item.id); detach(notification); };
        notification.onerror = () => { center.callback?.(JSON.stringify({kind: "failed", id: item.id, message: "Browser rejected notification"})); center.local.delete(item.id); detach(notification); };
        center.local.set(item.id, notification);
    }
}
function detach(notification) { notification.onclick = notification.onclose = notification.onerror = null; }
export async function remove(center, id) {
    if (center.registration) {
        const notifications = await center.registration.getNotifications({tag: center.app + ":" + id});
        for (const notification of notifications) notification.close();
    } else {
        const notification = center.local.get(id);
        if (notification) { detach(notification); notification.close(); center.local.delete(id); }
    }
}
export function close(center) {
    center.callback = null;
    if (center.listener) navigator.serviceWorker.removeEventListener("message", center.listener);
    for (const notification of center.local.values()) detach(notification);
    center.local.clear();
}
