// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * A sentence the core said, as the window starts one: its first letter capital, but purlis's
 * name, which is lowercase wherever it stands (`docs/ui-copy.md`). A leaf, so a surface that
 * says a core's reason imports it without the surfaces around it.
 */
export function sentence(said: string): string {
  if (/^purlis\b/u.test(said)) return said;
  return said.charAt(0).toUpperCase() + said.slice(1);
}
