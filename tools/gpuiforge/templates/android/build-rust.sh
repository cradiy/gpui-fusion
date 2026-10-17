#!/bin/sh
set -eu

output="${1:?Pass the output jniLibs directory}"
manifest="${2:?Pass the application Cargo.toml}"
library="${3:?Pass the native library name}"
forge="${4:-gpuiforge}"
abi="${GPUI_ANDROID_ABI:-arm64-v8a}"
case "$abi" in
    aarch64|aarch64-linux-android) abi=arm64-v8a ;;
    x86_64-linux-android) abi=x86_64 ;;
esac
exec "$forge" build-android-native --output "$output" --manifest-path "$manifest" \
    --library "$library" --abi "$abi" --profile "${GPUI_ANDROID_PROFILE:-debug}" \
    --min-sdk "${GPUI_ANDROID_MIN_SDK:-26}"
