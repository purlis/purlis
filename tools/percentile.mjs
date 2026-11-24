// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The `p` (0..1) percentile of `values` by nearest rank, or null for none. */
export function percentile(values, p) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.max(0, Math.ceil(p * sorted.length) - 1)];
}
