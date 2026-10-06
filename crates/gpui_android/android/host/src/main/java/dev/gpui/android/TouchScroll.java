package dev.gpui.android;

import android.content.Context;
import android.view.MotionEvent;
import android.view.VelocityTracker;
import android.view.ViewConfiguration;
import android.widget.OverScroller;

/** Single-contact scrolling synthesized only while raw touch handlers allow it. */
final class TouchScroll {
    private final GpuiSession session;
    private final OverScroller scroller;
    private final int touchSlop, minimumVelocity, maximumVelocity;
    private VelocityTracker velocity;
    private boolean blocked, dragging, scrolling;
    private float anchorX, anchorY, lastX, lastY;
    private int flingX, flingY;

    TouchScroll(Context context, GpuiSession session) {
        this.session = session;
        scroller = new OverScroller(context);
        ViewConfiguration config = ViewConfiguration.get(context);
        touchSlop = config.getScaledTouchSlop();
        minimumVelocity = config.getScaledMinimumFlingVelocity();
        maximumVelocity = config.getScaledMaximumFlingVelocity();
    }

    boolean begin(MotionEvent event) {
        boolean interrupted = scrolling;
        cancel();
        blocked = false;
        anchorX = lastX = event.getX();
        anchorY = lastY = event.getY();
        velocity = VelocityTracker.obtain();
        return interrupted;
    }

    void block() {
        blocked = true;
        finish(3);
        recycleVelocity();
    }

    // Returns true once a drag has crossed the slop, including its release.
    boolean event(MotionEvent event) {
        if (blocked || velocity == null) return false;
        velocity.addMovement(event);
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_MOVE) {
            float x = event.getX(), y = event.getY();
            if (!dragging && Math.hypot(x - anchorX, y - anchorY) > touchSlop) {
                dragging = scrolling = true;
                session.scroll(0, anchorX, anchorY, 0, 0);
            }
            if (dragging) session.scroll(1, anchorX, anchorY, x - lastX, y - lastY);
            lastX = x;
            lastY = y;
        } else if (action == MotionEvent.ACTION_UP) {
            boolean wasDragging = dragging;
            if (dragging) {
                velocity.computeCurrentVelocity(1000, maximumVelocity);
                float vx = velocity.getXVelocity(event.getPointerId(0));
                float vy = velocity.getYVelocity(event.getPointerId(0));
                if (Math.abs(vx) < minimumVelocity) vx = 0;
                if (Math.abs(vy) < minimumVelocity) vy = 0;
                if (vx != 0 || vy != 0) {
                    flingX = flingY = 0;
                    scroller.fling(0, 0, Math.round(vx), Math.round(vy),
                            Integer.MIN_VALUE / 2, Integer.MAX_VALUE / 2,
                            Integer.MIN_VALUE / 2, Integer.MAX_VALUE / 2);
                } else {
                    finish(2);
                }
            }
            dragging = false;
            recycleVelocity();
            return wasDragging;
        }
        return dragging;
    }

    void frame() {
        if (!scrolling || dragging) return;
        if (scroller.computeScrollOffset()) {
            int x = scroller.getCurrX(), y = scroller.getCurrY();
            if (x != flingX || y != flingY) {
                session.scroll(1, anchorX, anchorY, x - flingX, y - flingY);
            }
            flingX = x;
            flingY = y;
        }
        if (scroller.isFinished()) finish(2);
    }

    void cancel() {
        blocked = true;
        finish(3);
        recycleVelocity();
    }

    private void finish(int phase) {
        scroller.forceFinished(true);
        dragging = false;
        boolean wasScrolling = scrolling;
        scrolling = false;
        if (wasScrolling) session.scroll(phase, anchorX, anchorY, 0, 0);
    }

    private void recycleVelocity() {
        if (velocity != null) velocity.recycle();
        velocity = null;
    }
}
