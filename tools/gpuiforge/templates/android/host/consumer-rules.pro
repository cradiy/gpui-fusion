-keep class dev.gpui.android.GpuiSession { *; }
-keep class dev.gpui.android.TextInputState { *; }
# gpuiforge:if files
-keep class dev.gpui.android.SelectedDocument { *; }
-keep class dev.gpui.android.DocumentOutput { *; }
-keep class dev.gpui.android.FileStore { *; }
# gpuiforge:endif
# gpuiforge:if credentials
-keep class dev.gpui.android.CredentialStore { *; }
-keep class dev.gpui.android.StoredCredential { *; }
# gpuiforge:endif
# gpuiforge:if media
-keep class dev.gpui.android.MediaSession { *; }
# gpuiforge:endif
