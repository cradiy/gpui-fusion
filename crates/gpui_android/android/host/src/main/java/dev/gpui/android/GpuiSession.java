package dev.gpui.android;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.Intent;
import android.net.Uri;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.view.Surface;
import java.lang.ref.WeakReference;
import java.util.function.Consumer;

/** Owns a Rust application independently of its current View or Surface. */
public final class GpuiSession implements AutoCloseable {
    public static final int FOREGROUND = 0;
    public static final int ACTIVE = 1;
    public static final int INACTIVE = 2;
    public static final int BACKGROUND = 3;

    private final Handler handler = new Handler(Looper.getMainLooper());
    private WeakReference<GpuiView> view = new WeakReference<>(null);
    private volatile boolean closed;
    private long id;
    private int phase = BACKGROUND;
    private Runnable closeRequested;
    private Consumer<RuntimeException> errorHandler;

    static void checkThread() {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            throw new IllegalStateException("GpuiSession must be accessed on the main Looper");
        }
    }

    public GpuiSession() { checkThread(); }

    void bind(GpuiView next) {
        checkThread();
        if (closed) throw new IllegalStateException("GpuiSession is closed");
        GpuiView current = view.get();
        if (current != null && current != next) {
            throw new IllegalStateException("Detach the previous GpuiView before attaching another");
        }
        view = new WeakReference<>(next);
    }

    void unbind(GpuiView previous) {
        checkThread();
        if (view.get() == previous) view.clear();
    }

    void surface(Surface surface, int width, int height, float density) {
        checkThread();
        if (closed) return;
        if (id == 0) {
            id = nativeCreate(this, surface, width, height, density);
            nativeLifecycle(id, phase);
        } else {
            nativeAttach(id, surface, width, height, density);
        }
    }

    void detachSurface() { checkThread(); if (id != 0) nativeDetach(id); }
    boolean frame() { checkThread(); return id != 0 && nativeFrame(id); }
    TextInputState inputState() { checkThread(); return id != 0 ? nativeInputState(id) : null; }
    boolean edit(long epoch, int operation, String text, int a, int b) {
        checkThread();
        return !closed && id != 0 && nativeEdit(id, epoch, operation, text, a, b);
    }
    boolean key(String name, int modifiers, boolean down) {
        checkThread();
        return !closed && id != 0 && nativeKey(id, name, modifiers, down);
    }
    void focus(boolean focused) { checkThread(); if (id != 0) nativeFocus(id, focused); }
    boolean touch(int pointer, int phase, float x, float y) {
        return id != 0 && nativeTouch(id, pointer, phase, x, y);
    }
    void tap(float x, float y) { if (id != 0) nativeTap(id, x, y); }
    void scroll(int phase, float x, float y, float dx, float dy) {
        if (id != 0) nativeScroll(id, phase, x, y, dx, dy);
    }
    boolean active() { return !closed && phase == ACTIVE; }
    public boolean isClosed() { return closed; }

    /** Forward the host's onStart/onResume/onPause/onStop transitions. */
    public void setLifecycle(int next) {
        checkThread();
        if (next < FOREGROUND || next > BACKGROUND) throw new IllegalArgumentException("Invalid lifecycle phase");
        if (closed || next == phase) return;
        phase = next;
        if (id != 0) nativeLifecycle(id, phase);
        GpuiView current = view.get();
        if (current != null) current.updateFrameScheduling();
    }

    /** Supplies the host-specific action for a GPUI quit request. */
    public void setOnCloseRequested(Runnable callback) { checkThread(); closeRequested = callback; }

    /** Receives terminal rendering/initialization errors after the session closes. */
    public void setOnError(Consumer<RuntimeException> callback) { checkThread(); errorHandler = callback; }

    void fail(RuntimeException error) {
        Consumer<RuntimeException> callback = errorHandler;
        try { close(); } catch (RuntimeException closeError) { error.addSuppressed(closeError); }
        if (callback != null) callback.accept(error);
        else handler.post(() -> { throw error; });
    }

    // Called by Rust from foreground or background threads. Handler dispatch is
    // always asynchronous, so callbacks cannot re-enter a borrowed GPUI App.
    private void scheduleTask(long token, long delayMillis) {
        if (closed) return;
        handler.postAtTime(() -> {
            if (!closed && id != 0) {
                try { nativeRunTask(id, token); } catch (RuntimeException error) { fail(error); }
            }
        }, this, SystemClock.uptimeMillis() + delayMillis);
    }

    private void requestClose() {
        handler.postAtTime(() -> {
            if (!closed && closeRequested != null) closeRequested.run();
        }, this, SystemClock.uptimeMillis());
    }

    private Context requireContext() {
        checkThread();
        GpuiView current = view.get();
        if (closed || current == null) {
            throw new IllegalStateException("Android system services require an attached GpuiView");
        }
        return current.getContext();
    }

    private String readClipboard() {
        ClipboardManager clipboard = requireContext().getSystemService(ClipboardManager.class);
        ClipData clip = clipboard.getPrimaryClip();
        if (clip == null) return null;
        StringBuilder text = new StringBuilder();
        boolean found = false;
        for (int index = 0; index < clip.getItemCount(); index++) {
            CharSequence item = clip.getItemAt(index).getText();
            if (item == null) continue;
            if (found) text.append('\n');
            text.append(item);
            found = true;
        }
        return found ? text.toString() : null;
    }

    private void writeClipboard(String text) {
        ClipboardManager clipboard = requireContext().getSystemService(ClipboardManager.class);
        clipboard.setPrimaryClip(ClipData.newPlainText("", text));
    }

    private void openUrl(String url) {
        Context context = requireContext();
        Uri uri = Uri.parse(url);
        if (uri.getScheme() == null) throw new IllegalArgumentException("URL must have a scheme");
        Intent intent = new Intent(Intent.ACTION_VIEW, uri);
        Context owner = context;
        while (!(owner instanceof Activity) && owner instanceof ContextWrapper) {
            Context base = ((ContextWrapper) owner).getBaseContext();
            if (base == owner) break;
            owner = base;
        }
        if (!(owner instanceof Activity)) intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        context.startActivity(intent);
    }

    /** Releases the Rust application. Do not close during a retained Activity recreation. */
    @Override public void close() {
        checkThread();
        if (closed) return;
        closed = true;
        GpuiView current = view.get();
        try {
            if (current != null) current.releaseSurface();
        } finally {
            handler.removeCallbacksAndMessages(this);
            long previous = id;
            id = 0;
            try {
                if (previous != 0) nativeClose(previous);
            } finally {
                closeRequested = null;
                errorHandler = null;
                view.clear();
            }
        }
    }

    private static native long nativeCreate(GpuiSession host, Surface surface, int width, int height, float density);
    private static native void nativeAttach(long id, Surface surface, int width, int height, float density);
    private static native void nativeDetach(long id);
    private static native boolean nativeFrame(long id);
    private static native TextInputState nativeInputState(long id);
    private static native boolean nativeEdit(long id, long epoch, int operation, String text, int a, int b);
    private static native boolean nativeKey(long id, String name, int modifiers, boolean down);
    private static native void nativeLifecycle(long id, int phase);
    private static native void nativeFocus(long id, boolean active);
    private static native boolean nativeTouch(long id, int pointer, int phase, float x, float y);
    private static native void nativeTap(long id, float x, float y);
    private static native void nativeScroll(long id, int phase, float x, float y, float dx, float dy);
    private static native void nativeRunTask(long id, long token);
    private static native void nativeClose(long id);
}
