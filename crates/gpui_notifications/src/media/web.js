let active = null;
export function create_media(callback) {
    if (!navigator.mediaSession || !globalThis.MediaMetadata) throw new Error("Browser Media Session API is unavailable");
    if (active) throw new Error("A media session is already active; release it first");
    const owner = {callback, handlers: new Set(), metadata: "", artwork: null};
    active = owner;
    try {
        for (const action of ["play", "pause", "stop"]) bind(owner, action, () => callback(JSON.stringify({command: action})));
    } catch (error) { close_media(owner); throw error; }
    return owner;
}
function bind(owner, name, handler) {
    try { navigator.mediaSession.setActionHandler(name, handler); }
    catch (error) { if (error.name === "NotSupportedError") return; throw error; }
    if (handler) owner.handlers.add(name); else owner.handlers.delete(name);
}
function seconds(duration) { return duration.secs + duration.nanos / 1e9; }
function publish_metadata(owner, metadata) {
    navigator.mediaSession.metadata = new MediaMetadata({
        title: metadata.title, artist: metadata.artist || "", album: metadata.album || "",
        artwork: owner.artwork ? [{src: owner.artwork, type: "image/png"}] : [],
    });
}
export function set_artwork(owner, png) {
    if (active !== owner) throw new Error("Media session closed");
    const previous = owner.artwork;
    owner.artwork = png ? URL.createObjectURL(new Blob([Uint8Array.from(atob(png), c => c.charCodeAt(0))], {type: "image/png"})) : null;
    try { if (owner.metadata) publish_metadata(owner, JSON.parse(owner.metadata)); }
    catch (error) {
        if (owner.artwork) URL.revokeObjectURL(owner.artwork);
        owner.artwork = previous;
        throw error;
    }
    if (previous) URL.revokeObjectURL(previous);
}
export function update_media(owner, json) {
    if (active !== owner) throw new Error("Media session closed");
    const state = JSON.parse(json), session = navigator.mediaSession;
    const metadata = JSON.stringify(state.metadata);
    if (metadata !== owner.metadata) {
        publish_metadata(owner, state.metadata);
        owner.metadata = metadata;
    }
    session.playbackState = state.playback === "playing" || state.playback === "buffering" ? "playing" : state.playback === "paused" ? "paused" : "none";
    const callback = owner.callback;
    const duration = state.duration ? seconds(state.duration) : 0;
    if (session.setPositionState) {
        if (duration > 0) session.setPositionState({duration, position: Math.min(seconds(state.position), duration), playbackRate: state.rate});
        else session.setPositionState();
    }
    for (const [name, enabled, command] of [["nexttrack", state.can_next, "next"], ["previoustrack", state.can_previous, "previous"]]) {
        bind(owner, name, enabled ? () => callback(JSON.stringify({command})) : null);
    }
    bind(owner, "seekto", state.seekable ? event => { const secs = Math.max(0, event.seekTime); callback(JSON.stringify({command: "seek_to", value: {secs: Math.floor(secs), nanos: Math.floor((secs % 1) * 1e9)}})); } : null);
    for (const [name, sign] of [["seekforward", 1], ["seekbackward", -1]]) bind(owner, name, state.seekable ? event => callback(JSON.stringify({command: "seek_by", value: sign * (event.seekOffset || 10)})) : null);
}
export function close_media(owner) {
    if (active !== owner) return;
    for (const action of owner.handlers) navigator.mediaSession.setActionHandler(action, null);
    navigator.mediaSession.metadata = null;
    if (owner.artwork) URL.revokeObjectURL(owner.artwork);
    owner.artwork = null;
    navigator.mediaSession.playbackState = "none";
    if (navigator.mediaSession.setPositionState) navigator.mediaSession.setPositionState();
    owner.callback = null;
    active = null;
}
