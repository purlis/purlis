// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Tauri's build step, handed the probe's commands so the IPC is an allow-list, as the app's is
//! (ADR 0052): a command no capability grants is refused, which is what test 1 relies on.

fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["vault_reveal", "heard", "pong"]),
    ))
    .expect("tauri's build step");
}
