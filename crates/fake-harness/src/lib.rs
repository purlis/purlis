// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The fake harness's library half: the **fake provider** (`model`) every key-proxy test runs
//! against (design §5.4, R-695). The binary's `--model` serves through the same module.

pub mod model;
