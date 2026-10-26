// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A browser view's frames, drawn on a canvas** (#2012, spec #2008).
 *
 * The session host sends each frame whole, a JPEG; the window decodes it from its bytes and
 * draws it, so a page is never embedded in the window and its content policy is unchanged. A
 * view draws one frame at a time: one that arrives while another is being drawn waits, and
 * only the newest waits, so a view that falls behind skips to the latest whole frame.
 */

/** A frame's bytes, from the base64 the window is sent them in. */
export function frameBytes(base64: string): Uint8Array {
  const text = atob(base64);
  const bytes = new Uint8Array(text.length);
  for (let at = 0; at < text.length; at++) bytes[at] = text.charCodeAt(at);
  return bytes;
}

/**
 * Draws `jpeg` on `canvas`, at the frame's own size, or made `across` pixels wide (its height
 * kept in proportion) for a thumbnail, so its canvas holds no page-sized picture; the canvas
 * is scaled by its style. A screenshot of a browser timeline is a PNG (`type`, #2017).
 */
export async function paint(
  canvas: HTMLCanvasElement,
  jpeg: Uint8Array,
  across?: number,
  type = "image/jpeg",
): Promise<void> {
  const blob = new Blob([jpeg as BlobPart], { type });
  const bitmap = await (across === undefined
    ? createImageBitmap(blob)
    : createImageBitmap(blob, { resizeWidth: across, resizeQuality: "low" }));
  try {
    if (canvas.width !== bitmap.width) canvas.width = bitmap.width;
    if (canvas.height !== bitmap.height) canvas.height = bitmap.height;
    canvas.getContext("2d")?.drawImage(bitmap, 0, 0);
  } finally {
    bitmap.close();
  }
}

/**
 * What a view hands each frame to: `draw`, one frame at a time, the newest waiting while one is
 * drawn and every older one let go. A frame that cannot be drawn is passed over.
 */
export function latestOnly(
  draw: (frame: Uint8Array) => Promise<void>,
): (frame: Uint8Array) => void {
  let drawing = false;
  let waiting: Uint8Array | undefined;
  const next = (frame: Uint8Array) => {
    drawing = true;
    void draw(frame)
      .catch(() => undefined)
      .then(() => {
        const after = waiting;
        waiting = undefined;
        if (after === undefined) drawing = false;
        else next(after);
      });
  };
  return (frame) => {
    if (drawing) waiting = frame;
    else next(frame);
  };
}
