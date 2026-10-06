package dev.gpui.android;

final class TextInputState {
    final long epoch;
    final String text;
    final int offset, anchor, head, composingStart, composingEnd;
    final boolean hit;

    TextInputState(long epoch, String text, int offset, int anchor, int head,
                   int composingStart, int composingEnd, boolean hit) {
        this.epoch = epoch;
        this.text = text;
        this.offset = offset;
        this.anchor = anchor;
        this.head = head;
        this.composingStart = composingStart;
        this.composingEnd = composingEnd;
        this.hit = hit;
    }
}
