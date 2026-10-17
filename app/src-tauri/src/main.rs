// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Started as purlis's own wrap of a chat on Linux (#1040), the hidden `_sandbox`: become it
    // before anything of the app starts, so the program that starts a chat can wrap it.
    if let Some(code) = purlis_linux_sandbox::serve_if_asked() {
        std::process::exit(code);
    }
    // Started again by the session host as a browser engine's keeper (#2012): keep its process
    // group and never return, before anything of the app starts.
    #[cfg(unix)]
    purlis_core::browsers::keep_if_asked();
    // Started again by the window as its session host (#1997): serve until the window closes
    // its end, before anything of the app starts. The same binary, so the Keychain sees the
    // same program and asks nothing (ADR 0068 §1, amended 2026-10-14).
    #[cfg(unix)]
    if let Some(code) = purlis_app_lib::serve_as_the_session_host_if_asked() {
        std::process::exit(code);
    }
    // Started again as the bounded reader of a branch (FM-4, D-88h): answer one question and
    // exit, before anything of the app starts.
    if let Some(code) = purlis_core::files::serve_if_asked() {
        std::process::exit(code);
    }
    // Started again by git as purlis's credential helper, for a git the window runs (#752):
    // answer git from the forge account store and exit.
    if let Some(code) = purlis_core::forge::gitcredential::serve_if_asked() {
        std::process::exit(code);
    }
    // Started again by the `charter` command to write one keyring item, so the item is this
    // binary's and charter's app alone reads it without asking (ruling V90a).
    if let Some(code) = purlis_core::secrets::keyhold::serve_if_asked() {
        std::process::exit(code);
    }
    purlis_core::secrets::keyhold::this_process_is_the_app();
    purlis_app_lib::run()
}
