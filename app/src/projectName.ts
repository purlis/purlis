// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * A project's name: the last segment of its directory, as its tab on the title bar and a
 * terminal's prompt say it. A root (`/`, `C:\`) has no segment and is said whole.
 */
export function projectName(plane: string): string {
  const segments = plane.split(/[\\/]+/).filter((one) => one !== "");
  return segments.at(-1) ?? plane;
}
