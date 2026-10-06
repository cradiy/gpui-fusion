#!/usr/bin/env bash
set -euo pipefail

gpui_output="${1:?Pass the output jniLibs directory}"
gpui_app_manifest="${2:?Pass the application Cargo.toml}"
gpui_library="${3:?Pass the native library name}"
gpui_forge="${4:-gpuiforge}"
if [[ ! "$gpui_library" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
  printf 'Invalid native library name: %s\n' "$gpui_library" >&2; exit 1
fi
gpui_workspace="$(dirname "$(cargo locate-project --workspace --manifest-path "$gpui_app_manifest" --message-format plain)")"
gpui_profile="${GPUI_ANDROID_PROFILE:-debug}"
gpui_profile_args=()
case "$gpui_profile" in
  debug) ;;
  release) gpui_profile_args+=(--release) ;;
  *) printf 'Unsupported profile: %s\n' "$gpui_profile" >&2; exit 1 ;;
esac
gpui_api="${GPUI_ANDROID_MIN_SDK:-26}"
if [[ ! "$gpui_api" =~ ^[0-9]+$ ]] || (( gpui_api < 26 )); then
  printf 'Android minimum SDK must be at least 26.\n' >&2; exit 1
fi
gpui_ndk="${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to an installed Android NDK}"
case "$(uname -s)" in
  Linux) gpui_host=linux-x86_64 ;;
  Darwin) gpui_host=darwin-x86_64 ;;
  *) printf 'Android builds require a Linux or macOS host.\n' >&2; exit 1 ;;
esac
gpui_tools="$gpui_ndk/toolchains/llvm/prebuilt/$gpui_host/bin"
gpui_abi="${GPUI_ANDROID_ABI:-arm64-v8a}"
case "$gpui_abi" in
  aarch64|arm64-v8a|aarch64-linux-android) gpui_abi=arm64-v8a; gpui_target=aarch64-linux-android ;;
  x86_64|x86_64-linux-android) gpui_abi=x86_64; gpui_target=x86_64-linux-android ;;
  *) printf 'Unsupported Android ABI: %s\n' "$gpui_abi" >&2; exit 1 ;;
esac
if [[ ! -x "$gpui_tools/${gpui_target}${gpui_api}-clang" ]]; then
  printf 'Android NDK compiler not found: %s\n' "$gpui_tools/${gpui_target}${gpui_api}-clang" >&2
  exit 1
fi
if [[ ! -d "$(rustc --print target-libdir --target "$gpui_target")" ]]; then
  printf 'Rust target missing. Run: rustup target add %s\n' "$gpui_target" >&2
  exit 1
fi
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$gpui_tools/aarch64-linux-android${gpui_api}-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export CXX_aarch64_linux_android="$gpui_tools/aarch64-linux-android${gpui_api}-clang++"
export AR_aarch64_linux_android="$gpui_tools/llvm-ar"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$gpui_tools/x86_64-linux-android${gpui_api}-clang"
export CC_x86_64_linux_android="$CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER"
export CXX_x86_64_linux_android="$gpui_tools/x86_64-linux-android${gpui_api}-clang++"
export AR_x86_64_linux_android="$gpui_tools/llvm-ar"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$gpui_workspace/target}"
mkdir -p "$CARGO_TARGET_DIR"
export CARGO_TARGET_DIR="$(cd "$CARGO_TARGET_DIR" && pwd)"
gpui_manifest="$CARGO_TARGET_DIR/android/$gpui_library/$gpui_abi/Cargo.toml"
"$gpui_forge" prepare-android \
  --manifest-path "$gpui_app_manifest" \
  --workspace "$gpui_workspace" --output "$(dirname "$gpui_manifest")"
cargo rustc --manifest-path "$gpui_manifest" --lib --target "$gpui_target" "${gpui_profile_args[@]}" -- \
  -C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384
mkdir -p "$gpui_output/$gpui_abi"
cp "$CARGO_TARGET_DIR/$gpui_target/$gpui_profile/lib${gpui_library}.so" "$gpui_output/$gpui_abi/"
