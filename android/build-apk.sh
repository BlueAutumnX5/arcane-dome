#!/usr/bin/env bash
# Build the Android APK: target/android/arcane-dome-<version>.apk
#
# Needs the Android SDK (ANDROID_HOME) with an NDK, build-tools and a
# platform installed, a JDK, and the Rust targets:
#   rustup target add aarch64-linux-android armv7-linux-androideabi
#
# Signing: ARCANE_KEYSTORE (+ ARCANE_KEYSTORE_PASS, ARCANE_KEY_ALIAS) names
# the release key; without it the standard Android debug key is used
# (made in ~/.android/debug.keystore if missing).
set -euo pipefail
cd "$(dirname "$0")/.."

PACKAGE=io.github.blueautumnx5.arcanedome
LIB=arcane_dome
MIN_SDK=28    # Android 9
TARGET_SDK=34
TARGETS=${ANDROID_TARGETS:-"aarch64-linux-android armv7-linux-androideabi"}

SDK=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
[ -d "$SDK" ] || { echo "Set ANDROID_HOME to the Android SDK folder." >&2; exit 1; }
latest() { ls -d "$@" 2>/dev/null | sort -V | tail -n 1; }
NDK=${ANDROID_NDK_HOME:-$(latest "$SDK"/ndk/*)}
BUILD_TOOLS=$(latest "$SDK"/build-tools/*)
ANDROID_JAR=$(latest "$SDK"/platforms/android-*)/android.jar
for need in "$NDK" "$BUILD_TOOLS" "$ANDROID_JAR"; do
    [ -e "$need" ] || { echo "Missing from the SDK: $need" >&2; exit 1; }
done
TOOLCHAIN=$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
IFS=. read -r MAJOR MINOR PATCH <<< "$VERSION"
VERSION_CODE=$((MAJOR * 10000 + MINOR * 100 + PATCH))

OUT=target/android
WORK=$OUT/work
rm -rf "$WORK"
mkdir -p "$WORK/java" "$WORK/classes" "$WORK/dex" "$WORK/apk"

# 1. The game, as a shared library per CPU type.
for target in $TARGETS; do
    case $target in
        aarch64-linux-android) abi=arm64-v8a; clang=aarch64-linux-android$MIN_SDK-clang ;;
        armv7-linux-androideabi) abi=armeabi-v7a; clang=armv7a-linux-androideabi$MIN_SDK-clang ;;
        x86_64-linux-android) abi=x86_64; clang=x86_64-linux-android$MIN_SDK-clang ;;
        *) echo "Unknown target $target" >&2; exit 1 ;;
    esac
    upper=$(echo "$target" | tr 'a-z-' 'A-Z_')
    lower=${target//-/_}
    echo "== Building $target"
    env "CARGO_TARGET_${upper}_LINKER=$TOOLCHAIN/$clang" \
        "CARGO_TARGET_${upper}_RUSTFLAGS=-C link-arg=-Wl,-z,max-page-size=16384 -C strip=symbols" \
        "CC_${lower}=$TOOLCHAIN/$clang" "AR_${lower}=$TOOLCHAIN/llvm-ar" \
        cargo rustc --lib --release --target "$target" --crate-type cdylib
    mkdir -p "$WORK/apk/lib/$abi"
    cp "target/$target/release/lib$LIB.so" "$WORK/apk/lib/$abi/"
done

# 2. miniquad's Java activity, named for this app.
MINIQUAD=$(cargo metadata --format-version 1 |
    python3 -c 'import json, sys; print(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"] == "miniquad"))')
MINIQUAD=$(dirname "$MINIQUAD")
mkdir -p "$WORK/java/${PACKAGE//.//}" "$WORK/java/quad_native"
sed -e "s/TARGET_PACKAGE_NAME/$PACKAGE/" -e "s/LIBRARY_NAME/$LIB/" -e '/^[[:space:]]*\/\/%/d' \
    "$MINIQUAD/java/MainActivity.java" > "$WORK/java/${PACKAGE//.//}/MainActivity.java"
cp "$MINIQUAD/java/QuadNative.java" "$WORK/java/quad_native/"
echo "== Compiling Java"
javac -nowarn --release 8 -Xlint:-options -classpath "$ANDROID_JAR" -d "$WORK/classes" \
    $(find "$WORK/java" -name '*.java')
"$BUILD_TOOLS/d8" --release --min-api $MIN_SDK --lib "$ANDROID_JAR" --output "$WORK/dex" \
    $(find "$WORK/classes" -name '*.class')
cp "$WORK/dex/classes.dex" "$WORK/apk/"

# 3. Manifest and icon, then the code and libraries.
sed -e "s/@VERSION_CODE@/$VERSION_CODE/" -e "s/@VERSION_NAME@/$VERSION/" \
    -e "s/@MIN_SDK@/$MIN_SDK/" -e "s/@TARGET_SDK@/$TARGET_SDK/" \
    android/AndroidManifest.xml > "$WORK/AndroidManifest.xml"
"$BUILD_TOOLS/aapt2" compile --dir android/res -o "$WORK/res.zip"
"$BUILD_TOOLS/aapt2" link -o "$WORK/unsigned.apk" -I "$ANDROID_JAR" \
    --manifest "$WORK/AndroidManifest.xml" "$WORK/res.zip"
(cd "$WORK/apk" && zip -q -r ../unsigned.apk classes.dex lib)
"$BUILD_TOOLS/zipalign" -P 16 -f 4 "$WORK/unsigned.apk" "$WORK/aligned.apk"

# 4. Sign.
if [ -n "${ARCANE_KEYSTORE:-}" ]; then
    KEYSTORE=$ARCANE_KEYSTORE
    PASS=${ARCANE_KEYSTORE_PASS:?Set ARCANE_KEYSTORE_PASS}
    ALIAS=${ARCANE_KEY_ALIAS:?Set ARCANE_KEY_ALIAS}
else
    KEYSTORE=$HOME/.android/debug.keystore
    PASS=android
    ALIAS=androiddebugkey
    if [ ! -f "$KEYSTORE" ]; then
        mkdir -p "$HOME/.android"
        keytool -genkeypair -keystore "$KEYSTORE" -storepass "$PASS" -keypass "$PASS" \
            -alias "$ALIAS" -keyalg RSA -keysize 2048 -validity 10000 \
            -dname "CN=Android Debug,O=Android,C=US"
    fi
    echo "(Signed with the debug key: fine for testing, not for releases.)"
fi
APK=$OUT/arcane-dome-$VERSION.apk
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-pass "pass:$PASS" --ks-key-alias "$ALIAS" \
    --out "$APK" "$WORK/aligned.apk"
"$BUILD_TOOLS/apksigner" verify "$APK"
echo "== Built $APK ($(du -h "$APK" | cut -f1))"
