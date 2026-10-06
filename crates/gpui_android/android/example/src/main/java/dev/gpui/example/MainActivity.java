package dev.gpui.example;

import dev.gpui.android.GpuiActivity;

public final class MainActivity extends GpuiActivity {
    @Override protected String nativeLibraryName() { return "hello_android"; }
}
