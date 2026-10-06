#!/usr/bin/env bash
set -euo pipefail

gpui_root="$(cd "$(dirname "$0")/../.." && pwd)"
gpui_ndk="${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to an installed Android NDK}"
gpui_host="linux-x86_64"
if [[ "$(uname -s)" == Darwin ]]; then gpui_host="darwin-x86_64"; fi
gpui_tools="$gpui_ndk/toolchains/llvm/prebuilt/$gpui_host/bin"
gpui_abi="${GPUI_ANDROID_ABI:-arm64-v8a}"
case "$gpui_abi" in
  arm64-v8a) gpui_target=aarch64-linux-android ;;
  x86_64) gpui_target=x86_64-linux-android ;;
  *) printf 'Unsupported Android ABI: %s\n' "$gpui_abi" >&2; exit 1 ;;
esac
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$gpui_tools/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export CXX_aarch64_linux_android="$gpui_tools/aarch64-linux-android26-clang++"
export AR_aarch64_linux_android="$gpui_tools/llvm-ar"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$gpui_tools/x86_64-linux-android26-clang"
export CC_x86_64_linux_android="$CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER"
export CXX_x86_64_linux_android="$gpui_tools/x86_64-linux-android26-clang++"
export AR_x86_64_linux_android="$gpui_tools/llvm-ar"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$gpui_root/target}"
gpui_manifest="$CARGO_TARGET_DIR/android/hello_android/Cargo.toml"
cargo run --manifest-path "$gpui_root/Cargo.toml" -p gpui_android_build --locked -- prepare \
  --manifest-path "$gpui_root/crates/gpui_android/examples/hello_android/Cargo.toml" \
  --workspace "$gpui_root" --output "$(dirname "$gpui_manifest")"
cargo build --manifest-path "$gpui_manifest" --lib --target "$gpui_target"
gpui_output="${1:?Pass the output jniLibs directory}"
mkdir -p "$gpui_output/$gpui_abi"
cp "$CARGO_TARGET_DIR/$gpui_target/debug/libhello_android.so" "$gpui_output/$gpui_abi/"
