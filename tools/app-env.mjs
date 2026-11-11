// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The environment a relaunch tool gives the app it starts (#5691).
//
// The operator runs these tools from a shell inside a purlis chat, and that shell carries the
// chat's own product variables, under both names: its plane (`PURLIS_ROOT`), its token, its hook
// socket, its persona. Spread into the app's environment, they point the app under test at the
// operator's plane and live chat, and an inherited `PURLIS_ROOT` beside the tool's
// `CHARTER_ROOT` is refused outright ("PURLIS_ROOT and CHARTER_ROOT disagree"). So no product
// variable of the parent's goes on: the app gets the parent's environment without any
// `PURLIS_*` or `CHARTER_*`, and then only what the tool sets.

const PRODUCT = /^(PURLIS|CHARTER)_/;

/** `parent` without its product variables, with `set` over it. */
export function appEnv(parent, set) {
  const kept = Object.fromEntries(
    Object.entries(parent).filter(([name]) => !PRODUCT.test(name)),
  );
  return { ...kept, ...set };
}
