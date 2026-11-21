#!/bin/bash
# Builds Noctiluca.app: the visualizer as a Mac app that opens with a
# double-click, with its shaders and models inside it.
#
#     ./package_app.sh                 (to ~/Documents/Noctiluca.app)
#     ./package_app.sh /some/folder    (to /some/folder/Noctiluca.app)
#     PUBLIC=1 ./package_app.sh DIR    (a build to share: see below)

set -e
cd "$(dirname "$0")"

DEST="${1:-$HOME/Documents}"
APP="$DEST/Noctiluca.app"
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"

# PUBLIC=1: a build to give to others. It leaves out adb (below) and keeps
# the paths of this computer out of the program (Rust keeps the source
# path of every place that can panic), in its own target folder so the
# everyday build is not thrown away.
TARGET=target
if [ "${PUBLIC:-0}" = 1 ]; then
    TARGET=target/public
    # The encoded form (flags joined by the 0x1f character) keeps paths
    # with spaces in one piece.
    US=$'\x1f'
    export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$HOME/.cargo/registry/src=/cargo${US}--remap-path-prefix=$HOME/.rustup=/rustup${US}--remap-path-prefix=$PWD=/noctiluca"
fi
CARGO_TARGET_DIR="$TARGET" cargo build --release --quiet

# The Noctiluca Screen app for Android TV boxes (android/), built when Java
# and Gradle are here, so the menu can install it on boxes on the network.
# Java: $JAVA_HOME, else Android Studio's own, else the system's.
# Gradle: the 8.x Android Studio downloaded (what the Android plugin is made
# for), else the one on the PATH.
JBR="$JAVA_HOME"
for j in "$HOME"/Library/Java/JavaVirtualMachines/jbr-*/Contents/Home "/Applications/Android Studio.app/Contents/jbr/Contents/Home"; do
    [ -d "$JBR" ] || JBR="$j"
done
[ -d "$JBR" ] || JBR="$(/usr/libexec/java_home 2>/dev/null || true)"
GRADLE="$(ls -d "$HOME"/.gradle/wrapper/dists/gradle-8.*-bin/*/gradle-8.*/bin/gradle 2>/dev/null | tail -1)"
[ -n "$GRADLE" ] || GRADLE="$(command -v gradle || true)"
if [ -n "$GRADLE" ] && [ -x "$GRADLE" ] && [ -d "$JBR" ]; then
    (cd android && JAVA_HOME="$JBR" "$GRADLE" --offline -q assembleRelease) || echo "(the box app was not rebuilt; using the last one)"
else
    echo "(no Java or Gradle here: the app is built without the TV box receiver)"
fi
APK=android/app/build/outputs/apk/release/app-release.apk
ADB="${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools/adb"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$TARGET/release/noctiluca" "$APP/Contents/MacOS/noctiluca"
cp -R shaders "$APP/Contents/Resources/shaders"
cp -R assets/models "$APP/Contents/Resources/models"
cp assets/icon/Noctiluca.icns "$APP/Contents/Resources/Noctiluca.icns"
[ -f "$APK" ] && cp "$APK" "$APP/Contents/Resources/NoctilucaScreen.apk"
# adb comes from Google's Android SDK, whose license does not allow passing
# it on: a public build leaves it out, and the app then uses the one from
# `brew install android-platform-tools`.
if [ "${PUBLIC:-0}" != 1 ] && [ -x "$ADB" ]; then cp "$ADB" "$APP/Contents/Resources/adb"; fi

# What macOS needs to know about the app. NSMicrophoneUsageDescription is
# required: without it macOS silently refuses the app the microphone.
cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>                  <string>Noctiluca</string>
    <key>CFBundleDisplayName</key>           <string>Noctiluca</string>
    <key>CFBundleIdentifier</key>            <string>org.noctiluca.app</string>
    <key>CFBundleExecutable</key>            <string>noctiluca</string>
    <key>CFBundleIconFile</key>              <string>Noctiluca</string>
    <key>CFBundlePackageType</key>           <string>APPL</string>
    <key>CFBundleShortVersionString</key>    <string>$VERSION</string>
    <key>CFBundleVersion</key>               <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>        <string>13.0</string>
    <key>NSHighResolutionCapable</key>       <true/>
    <key>NSMicrophoneUsageDescription</key>  <string>Noctiluca listens to the music in the room to move the visuals with it.</string>
    <key>NSLocalNetworkUsageDescription</key> <string>Noctiluca finds the TV boxes on your network and streams the visuals to them.</string>
    <key>NSBonjourServices</key>
    <array>
        <string>_noctiluca._tcp</string>
        <string>_androidtvremote2._tcp</string>
    </array>
</dict>
</plist>
EOF

# A local signature: macOS remembers the microphone permission for a signed
# app, instead of asking again after every rebuild of an unsigned one.
codesign --force --deep --sign - "$APP" 2>/dev/null
# Tell Finder the app changed, so it shows the new icon at once.
touch "$APP"

echo "Built $APP ($(du -sh "$APP" | cut -f1))"
echo "Double-click it to start. It opens on Live: Start the show (or Space) plays, Esc comes back to Live, Cmd+Q quits."
