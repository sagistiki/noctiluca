# Noctiluca Screen (Android TV)

The app for Android TV boxes that makes a box a Noctiluca output: it waits
on the network, and plays what Noctiluca on the Mac streams to it (H.264 from
the Mac's encoder, decoded in the box's hardware, each frame shown at the
moment the Mac asks for).

Install it from Noctiluca: Network screens → Install (turn on network
debugging on the box first: Settings → Device preferences → Developer options). Or by hand:

```bash
./package_app.sh   # from the project folder: builds this app too and puts it inside Noctiluca.app
```

Build alone (Java 17 or later, the Android SDK with `ANDROID_HOME` or a
`local.properties` naming it, Gradle 8.9):

```bash
gradle assembleRelease   # → app/build/outputs/apk/release/app-release.apk
```

- Plain Android (no libraries): starts fast on a 2 GB box. Android 8 and up.
- Signing: release builds use the key named in `keystore.properties` here
  (not in the repository: `storeFile`, `storePassword`, `keyAlias`,
  `keyPassword`), so updates install over the old app. Without that file the
  build signs with Android's debug key, which is fine for your own boxes.
- Version: `versionName` in app/build.gradle.kts and `BuildConfigVersion` in
  Receiver.kt, and `SHIPPED_VERSION` in ../src/net/adb.rs (the Mac offers an
  update to boxes with an older one). Change all three together.
- Protocol: Protocol.kt here and ../src/net/protocol.rs must match.
