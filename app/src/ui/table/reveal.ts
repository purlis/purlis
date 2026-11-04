// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Where a frame is scrolled sideways to show a control and its ring**, from where it stood
 * when the focus came (`start`): unmoved where they are in sight already, else just enough that
 * the nearer edge is in, the ring's width inside it, kept within `0..most`. `left` and `right`
 * are the control's edges in the frame's content; `room` the frame's inner width.
 */
export function revealedScrollLeft({
  start,
  left,
  right,
  room,
  most,
  ring,
}: {
  start: number;
  left: number;
  right: number;
  room: number;
  most: number;
  ring: number;
}): number {
  const from = left - ring;
  const to = right + ring;
  let at = start;
  if (from < start) at = from;
  else if (to > start + room) at = to - room;
  return Math.round(Math.min(Math.max(at, 0), Math.max(most, 0)));
}
