// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **How long a check or a step took**, in words: `0.3 s`, `12 s`, `1 min 4 s`. */
export function tookWords(ms: number | null | undefined): string | undefined {
  if (ms === null || ms === undefined || !Number.isFinite(ms) || ms < 0) return undefined;
  if (ms < 10_000) return `${(Math.round(ms / 100) / 10).toFixed(1)} s`;
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds} s`;
  const minutes = Math.floor(seconds / 60);
  const rest = seconds % 60;
  return rest === 0 ? `${minutes} min` : `${minutes} min ${rest} s`;
}
