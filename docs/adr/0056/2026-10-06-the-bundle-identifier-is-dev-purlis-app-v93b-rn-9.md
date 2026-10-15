# Amended 2026-10-06: the bundle identifier is `dev.purlis.app` (V93b, RN-9)

The product was renamed purlis (V93a), and ruling V93b moves the app with it: the bundle
identifier is now `dev.purlis.app` and the product name `purlis` (issue 1267). This replaces the
two lines above that keep `dev.charter.app`. At its first launch under the new identifier the app
moves its log folder and holds its keychain items again; on Linux the one-per-user lock and the
D-Bus name follow the identifier, and the `.deb` replaces the `charter` package. The old
identifier stays one the app recognises (`names::BUNDLE_ID`), so a charter still running under it
is seen.

The binary is still `charter-app`, and so is the Tauri crate: `keyhold::APP_BINARY`, the scenario
configs and the tools find the binary by that name, and it moves with the crates (RN-13).
