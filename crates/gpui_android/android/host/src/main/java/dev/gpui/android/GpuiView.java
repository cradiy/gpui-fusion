package dev.gpui.android;

import android.content.Context;
import android.graphics.PointF;
import android.util.SparseArray;
import android.view.Choreographer;
import android.view.MotionEvent;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.ViewConfiguration;
import android.view.View;
import android.view.KeyEvent;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.text.InputType;

/** A GPUI rendering surface. The caller owns and eventually closes its session. */
public final class GpuiView extends SurfaceView implements SurfaceHolder.Callback, Choreographer.FrameCallback {
    private final GpuiSession session;
    private final Choreographer choreographer;
    private final TouchScroll scroll;
    private final SparseArray<PointF> contacts = new SparseArray<>();
    private final int touchSlop;
    private boolean surfaceReady;
    private boolean framePosted;
    private boolean tapCandidate;
    private float downX, downY, tapX, tapY;
    private TextInputState inputState;
    private GpuiInputConnection inputConnection;
    private boolean requestKeyboard;

    public GpuiView(Context context, GpuiSession session) {
        super(context);
        GpuiSession.checkThread();
        this.session = session;
        choreographer = Choreographer.getInstance();
        scroll = new TouchScroll(context, session);
        touchSlop = ViewConfiguration.get(context).getScaledTouchSlop();
        getHolder().addCallback(this);
        setFocusableInTouchMode(true);
        setClickable(true);
    }

    @Override protected void onAttachedToWindow() {
        super.onAttachedToWindow();
        session.bind(this);
    }

    @Override protected void onDetachedFromWindow() {
        releaseSurface();
        session.unbind(this);
        super.onDetachedFromWindow();
    }

    @Override public void surfaceCreated(SurfaceHolder holder) {}

    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        if (width <= 0 || height <= 0) { releaseSurface(); return; }
        cancelTouches();
        try {
            session.surface(holder.getSurface(), width, height, getResources().getDisplayMetrics().density);
        } catch (RuntimeException error) {
            session.fail(error);
            return;
        }
        surfaceReady = true;
        updateFrameScheduling();
    }

    @Override public void surfaceDestroyed(SurfaceHolder holder) { releaseSurface(); }

    void releaseSurface() {
        closeInput();
        choreographer.removeFrameCallback(this);
        framePosted = false;
        cancelTouches();
        if (surfaceReady) {
            surfaceReady = false;
            session.focus(false);
            session.detachSurface();
        }
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        updateFrameScheduling();
    }

    @Override protected void onVisibilityChanged(View changedView, int visibility) {
        super.onVisibilityChanged(changedView, visibility);
        if (session != null) updateFrameScheduling();
    }

    @Override protected void onWindowVisibilityChanged(int visibility) {
        super.onWindowVisibilityChanged(visibility);
        if (session != null) updateFrameScheduling();
    }

    void updateFrameScheduling() {
        boolean active = surfaceReady && session.active() && hasWindowFocus() && isShown();
        session.focus(active);
        if (active && !framePosted) {
            framePosted = true;
            choreographer.postFrameCallback(this);
        } else if (!active) {
            choreographer.removeFrameCallback(this);
            framePosted = false;
            cancelTouches();
        }
    }

    @Override public void doFrame(long frameTimeNanos) {
        framePosted = false;
        if (surfaceReady && session.active() && hasWindowFocus() && isShown()) {
            try {
                scroll.frame();
                if (session.frame() || requestKeyboard) syncInput(requestKeyboard);
            } catch (RuntimeException error) { session.fail(error); return; }
        }
        updateFrameScheduling();
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (!surfaceReady || !session.active()) return false;
        try {
            return dispatchTouch(event);
        } catch (RuntimeException error) {
            session.fail(error);
            return true;
        }
    }

    private boolean dispatchTouch(MotionEvent event) {
        int action = event.getActionMasked();
        int index = event.getActionIndex();
        if (action == MotionEvent.ACTION_DOWN) {
            requestFocus();
            tapCandidate = !scroll.begin(event);
            downX = event.getX(); downY = event.getY();
        }
        if (event.getPointerCount() > 1) {
            tapCandidate = false;
            scroll.block();
        }
        if (action == MotionEvent.ACTION_CANCEL) {
            cancelTouches();
            return true;
        }
        if (action == MotionEvent.ACTION_MOVE) {
            for (int i = 0; i < event.getPointerCount(); i++) {
                updateTouch(event, i, 1);
            }
            if (Math.hypot(event.getX() - downX, event.getY() - downY) > touchSlop) tapCandidate = false;
            if (scroll.event(event)) tapCandidate = false;
        } else if (action == MotionEvent.ACTION_DOWN || action == MotionEvent.ACTION_POINTER_DOWN) {
            updateTouch(event, index, 0);
            scroll.event(event);
        } else if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_POINTER_UP) {
            updateTouch(event, index, 2);
            contacts.remove(event.getPointerId(index));
            if (scroll.event(event)) tapCandidate = false;
            if (action == MotionEvent.ACTION_UP) {
                if (tapCandidate && event.getEventTime() - event.getDownTime() < ViewConfiguration.getLongPressTimeout()
                        && Math.hypot(event.getX() - downX, event.getY() - downY) <= touchSlop) {
                    tapX = event.getX(); tapY = event.getY();
                    performClick();
                }
                tapCandidate = false;
            }
        }
        return true;
    }

    private void updateTouch(MotionEvent event, int index, int phase) {
        int id = event.getPointerId(index);
        float x = event.getX(index), y = event.getY(index);
        contacts.put(id, new PointF(x, y));
        if (session.touch(id, phase, x, y)) {
            tapCandidate = false;
            scroll.block();
        }
    }

    private void cancelTouches() {
        tapCandidate = false;
        scroll.cancel();
        for (int i = 0; i < contacts.size(); i++) {
            PointF point = contacts.valueAt(i);
            session.touch(contacts.keyAt(i), 3, point.x, point.y);
        }
        contacts.clear();
    }

    @Override public boolean performClick() {
        super.performClick();
        closeInput();
        session.tap(tapX, tapY);
        requestKeyboard = true;
        return true;
    }

    InputMethodManager inputManager() { return getContext().getSystemService(InputMethodManager.class); }

    private void closeInput() {
        if (inputConnection != null) {
            GpuiInputConnection previous = inputConnection;
            inputConnection = null;
            previous.closeConnection();
        }
        inputState = null;
        requestKeyboard = false;
    }

    void syncInput(boolean show) {
        requestKeyboard = false;
        if (!surfaceReady || !hasWindowFocus()) return;
        if (inputConnection != null && inputConnection.batching()) return;
        TextInputState next = session.inputState();
        boolean changed = (next == null) != (inputState == null)
                || (next != null && inputState != null && next.epoch != inputState.epoch);
        if (changed) {
            closeInput();
            inputState = next;
            inputManager().restartInput(this);
            if (next == null) inputManager().hideSoftInputFromWindow(getWindowToken(), 0);
        } else inputState = next;
        if (next != null) {
            inputManager().updateSelection(this, next.anchor, next.head, next.composingStart, next.composingEnd);
            if (inputConnection != null) inputConnection.updateExtracted(next);
            if (show && next.hit) inputManager().showSoftInput(this, InputMethodManager.SHOW_IMPLICIT);
        }
    }

    @Override public boolean onCheckIsTextEditor() { return inputState != null; }

    @Override public InputConnection onCreateInputConnection(EditorInfo info) {
        if (inputState == null || !surfaceReady) return null;
        if (inputConnection != null) inputConnection.closeConnection();
        inputState = session.inputState();
        if (inputState == null) return null;
        info.inputType = InputType.TYPE_CLASS_TEXT | (inputState.text == null
                ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        info.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING;
        info.initialSelStart = inputState.anchor;
        info.initialSelEnd = inputState.head;
        if (android.os.Build.VERSION.SDK_INT >= 30 && inputState.text != null) {
            info.setInitialSurroundingSubText(inputState.text, inputState.offset);
        }
        inputConnection = GpuiInputConnection.create(this, session, inputState.epoch);
        return inputConnection;
    }

    @Override public boolean onKeyDown(int code, KeyEvent event) {
        return handleKey(code, event, true) || super.onKeyDown(code, event);
    }
    @Override public boolean onKeyUp(int code, KeyEvent event) {
        return handleKey(code, event, false) || super.onKeyUp(code, event);
    }
    private boolean handleKey(int code, KeyEvent event, boolean down) {
        if (!surfaceReady) return false;
        String key;
        switch (code) {
            case KeyEvent.KEYCODE_DEL: key = "backspace"; break;
            case KeyEvent.KEYCODE_FORWARD_DEL: key = "delete"; break;
            case KeyEvent.KEYCODE_ENTER: key = "enter"; break;
            case KeyEvent.KEYCODE_DPAD_LEFT: key = "left"; break;
            case KeyEvent.KEYCODE_DPAD_RIGHT: key = "right"; break;
            case KeyEvent.KEYCODE_DPAD_UP: key = "up"; break;
            case KeyEvent.KEYCODE_DPAD_DOWN: key = "down"; break;
            case KeyEvent.KEYCODE_MOVE_HOME: key = "home"; break;
            case KeyEvent.KEYCODE_MOVE_END: key = "end"; break;
            default:
                int character = event.getUnicodeChar();
                if (event.isCtrlPressed() || event.isMetaPressed()) {
                    character = event.getUnicodeChar(0);
                    if (character == 0) return false;
                    key = new String(Character.toChars(character));
                } else {
                    if (character == 0 || (character & android.view.KeyCharacterMap.COMBINING_ACCENT) != 0 || inputState == null) return false;
                    if (down) session.edit(inputState.epoch, 0, new String(Character.toChars(character)), 1, 0);
                    syncInput(false);
                    return true;
                }
        }
        int modifiers = (event.isShiftPressed() ? 1 : 0) | (event.isCtrlPressed() ? 2 : 0)
                | (event.isAltPressed() ? 4 : 0) | (event.isMetaPressed() ? 8 : 0);
        session.key(key, modifiers, down);
        syncInput(false);
        return true;
    }
}
