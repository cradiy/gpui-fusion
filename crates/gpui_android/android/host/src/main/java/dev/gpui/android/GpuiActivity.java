package dev.gpui.android;

import android.app.Activity;
import android.os.Bundle;
import android.os.Build;
import android.graphics.Insets;
import android.view.WindowInsets;
import android.widget.FrameLayout;
import android.widget.TextView;

/** Full-page host retaining the Rust application across configuration changes. */
public abstract class GpuiActivity extends Activity {
    private GpuiSession session;
    protected abstract String nativeLibraryName();

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        System.loadLibrary(nativeLibraryName());
        Object retained = getLastNonConfigurationInstance();
        session = retained instanceof GpuiSession ? (GpuiSession) retained : new GpuiSession();
        session.setOnCloseRequested(this::finish);
        session.setOnError(error -> {
            android.util.Log.e("GPUI", "Unable to display GPUI", error);
            TextView message = new TextView(this);
            int padding = Math.round(24 * getResources().getDisplayMetrics().density);
            message.setPadding(padding, padding * 3, padding, padding);
            message.setText("Unable to start GPUI\n\n" + error.getMessage());
            setContentView(message);
        });
        FrameLayout content = new FrameLayout(this);
        content.addView(new GpuiView(this, session), new FrameLayout.LayoutParams(-1, -1));
        content.setOnApplyWindowInsetsListener((view, insets) -> {
            if (Build.VERSION.SDK_INT >= 30) {
                Insets safe = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
                Insets keyboard = insets.getInsets(WindowInsets.Type.ime());
                view.setPadding(safe.left, safe.top, safe.right, Math.max(safe.bottom, keyboard.bottom));
            } else {
                view.setPadding(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
                        insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
            }
            return insets;
        });
        setContentView(content);
        content.requestApplyInsets();
    }
    @Override public Object onRetainNonConfigurationInstance() { return session.isClosed() ? null : session; }
    @Override protected void onStart() { super.onStart(); session.setLifecycle(GpuiSession.FOREGROUND); }
    @Override protected void onResume() { super.onResume(); session.setLifecycle(GpuiSession.ACTIVE); }
    @Override protected void onPause() { session.setLifecycle(GpuiSession.INACTIVE); super.onPause(); }
    @Override protected void onStop() { session.setLifecycle(GpuiSession.BACKGROUND); super.onStop(); }
    @Override protected void onDestroy() {
        session.setOnCloseRequested(null);
        session.setOnError(null);
        if (!isChangingConfigurations()) session.close();
        super.onDestroy();
    }
}
