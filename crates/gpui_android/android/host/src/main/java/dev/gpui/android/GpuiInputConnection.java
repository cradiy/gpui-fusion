package dev.gpui.android;

import android.view.KeyEvent;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.ExtractedTextRequest;
import android.view.inputmethod.InputConnection;

class GpuiInputConnection extends BaseInputConnection {
    private final GpuiView view;
    private final GpuiSession session;
    private final long epoch;
    private boolean closed;
    private int batches;
    private int extractedToken;
    private boolean monitorExtracted;

    GpuiInputConnection(GpuiView view, GpuiSession session, long epoch) {
        super(view, true);
        this.view = view;
        this.session = session;
        this.epoch = epoch;
    }

    @Override public android.text.Editable getEditable() { return null; }

    static GpuiInputConnection create(GpuiView view, GpuiSession session, long epoch) {
        if (android.os.Build.VERSION.SDK_INT >= 34) return new Api34Connection(view, session, epoch);
        return new GpuiInputConnection(view, session, epoch);
    }

    @android.annotation.TargetApi(34)
    private static final class Api34Connection extends GpuiInputConnection {
        Api34Connection(GpuiView view, GpuiSession session, long epoch) { super(view, session, epoch); }

        @Override public boolean replaceText(int start, int end, CharSequence text, int cursor,
                                             android.view.inputmethod.TextAttribute attributes) {
            if (start < 0 || end < 0 || !beginBatchEdit()) return false;
            try {
                return finishComposingText() && setSelection(Math.min(start, end), Math.max(start, end))
                        && commitText(text, cursor);
            } finally { endBatchEdit(); }
        }
    }

    TextInputState state() {
        if (closed) return null;
        TextInputState state = session.inputState();
        return state != null && state.epoch == epoch ? state : null;
    }

    private boolean edit(int operation, String text, int a, int b) {
        if (closed) return false;
        try {
            boolean result = session.edit(epoch, operation, text, a, b);
            if (batches == 0) view.syncInput(false);
            return result;
        } catch (RuntimeException error) {
            session.fail(error);
            return false;
        }
    }

    @Override public boolean commitText(CharSequence text, int cursor) { return edit(0, text.toString(), cursor, 0); }
    @Override public boolean setComposingText(CharSequence text, int cursor) { return edit(1, text.toString(), cursor, 0); }
    @Override public boolean finishComposingText() { return edit(2, "", 0, 0); }
    @Override public boolean setSelection(int start, int end) { return edit(3, "", start, end); }
    @Override public boolean setComposingRegion(int start, int end) { return edit(4, "", start, end); }
    @Override public boolean deleteSurroundingText(int before, int after) { return edit(5, "", before, after); }
    @Override public boolean deleteSurroundingTextInCodePoints(int before, int after) { return edit(6, "", before, after); }

    @Override public boolean beginBatchEdit() { if (closed) return false; batches++; return true; }
    @Override public boolean endBatchEdit() {
        if (closed || batches == 0) return false;
        if (--batches == 0) view.syncInput(false);
        return batches > 0;
    }
    boolean batching() { return batches > 0; }

    @Override public void closeConnection() {
        if (closed) return;
        // Invalidate first: finishing composition can cause a focus refresh.
        closed = true;
        batches = 0;
        try { session.edit(epoch, 2, "", 0, 0); }
        finally { super.closeConnection(); }
    }

    private CharSequence slice(TextInputState state, int start, int end) {
        if (state == null || state.text == null) return null;
        start = Math.max(0, Math.min(state.text.length(), start - state.offset));
        end = Math.max(start, Math.min(state.text.length(), end - state.offset));
        // A UTF-16 limit must not expose half a surrogate pair.
        if (start > 0 && start < state.text.length() && Character.isLowSurrogate(state.text.charAt(start))) start++;
        if (end > start && end < state.text.length() && Character.isHighSurrogate(state.text.charAt(end - 1))) end--;
        return state.text.substring(start, end);
    }

    @Override public CharSequence getTextBeforeCursor(int length, int flags) {
        TextInputState state = state();
        if (state == null || length < 0) return null;
        int end = Math.min(state.anchor, state.head);
        return slice(state, Math.max(0, end - length), end);
    }
    @Override public CharSequence getTextAfterCursor(int length, int flags) {
        TextInputState state = state();
        if (state == null || length < 0) return null;
        int start = Math.max(state.anchor, state.head);
        return slice(state, start, (int) Math.min(Integer.MAX_VALUE, (long) start + length));
    }
    @Override public CharSequence getSelectedText(int flags) {
        TextInputState state = state();
        return state == null ? null : slice(state, Math.min(state.anchor, state.head), Math.max(state.anchor, state.head));
    }
    @Override public int getCursorCapsMode(int modes) {
        TextInputState state = state();
        if (state == null || state.text == null) return 0;
        int cursor = Math.max(0, Math.min(state.text.length(), state.head - state.offset));
        return android.text.TextUtils.getCapsMode(state.text, cursor, modes);
    }

    private ExtractedText extracted(TextInputState state) {
        if (state == null || state.text == null) return null;
        ExtractedText result = new ExtractedText();
        result.text = state.text;
        result.startOffset = state.offset;
        result.partialStartOffset = result.partialEndOffset = -1;
        result.selectionStart = state.anchor - state.offset;
        result.selectionEnd = state.head - state.offset;
        return result;
    }
    @Override public ExtractedText getExtractedText(ExtractedTextRequest request, int flags) {
        if (request == null || closed) return null;
        monitorExtracted = (flags & InputConnection.GET_EXTRACTED_TEXT_MONITOR) != 0;
        extractedToken = request.token;
        return extracted(state());
    }
    void updateExtracted(TextInputState state) {
        ExtractedText text = extracted(state);
        if (monitorExtracted && text != null) view.inputManager().updateExtractedText(view, extractedToken, text);
    }

    @Override public boolean sendKeyEvent(KeyEvent event) { return !closed && view.dispatchKeyEvent(event); }
    @Override public boolean performEditorAction(int action) {
        if (closed) return false;
        session.key("enter", 0, true);
        session.key("enter", 0, false);
        view.syncInput(false);
        return true;
    }
    @Override public boolean performContextMenuAction(int id) {
        String key;
        if (id == android.R.id.selectAll) key = "a";
        else if (id == android.R.id.copy) key = "c";
        else if (id == android.R.id.cut) key = "x";
        else if (id == android.R.id.paste) key = "v";
        else return false;
        if (closed) return false;
        session.key(key, 2, true);
        session.key(key, 2, false);
        view.syncInput(false);
        return true;
    }
}
