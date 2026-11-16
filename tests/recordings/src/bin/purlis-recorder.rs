// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The recorder a recording's hook entries name in place of `purlis` (T1.1-03):
//! [`purlis_recordings::recorder`].

fn main() -> std::process::ExitCode {
    let code = purlis_recordings::recorder::run(std::env::args().skip(1).collect());
    std::process::ExitCode::from(u8::try_from(code).unwrap_or(1))
}
