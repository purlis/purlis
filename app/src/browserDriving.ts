// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { BrowserInput, MouseButton } from "./bindings";

/**
 * **Driving a browser from its view** (#2014, spec #2008).
 *
 * Clicking or typing into a browser view takes the browser over at once: from then, the
 * person's clicks, moves, wheel and keys go to the page, each aimed at what the frame they saw
 * showed (its `aim`), and the agent's browser tools are refused in words. Only the person hands
 * it back: Done, or the one fixed chord ({@link handsBack}). What purlis claims on the window
 * stays purlis's while they drive, as over a terminal: a Control chord stays with the page
 * (`CHAT_KEYBOARD`), and the palette's pass-through key, pressed again, reaches the page
 * ({@link sendToDriven}).
 */

/** A page size the person can pick, in CSS pixels. */
export type SizePreset = { id: string; label: string; width: number; height: number };

/** The size presets, the engine's own size first. */
export const SIZE_PRESETS: readonly SizePreset[] = [
  { id: "laptop", label: "Laptop", width: 1280, height: 800 },
  { id: "desktop", label: "Desktop", width: 1920, height: 1080 },
  { id: "tablet", label: "Tablet", width: 820, height: 1180 },
  { id: "phone", label: "Phone", width: 390, height: 844 },
];

/** The keys held, as the bits the session host reads: Alt 1, Control 2, Meta 4, Shift 8. */
export function modifiersOf(e: {
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}): number {
  return (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0);
}

/**
 * **The one chord that hands the browser back**: ⌘⇧Enter on a Mac, Ctrl+Shift+Enter elsewhere.
 * A page has no use for it, and it is never sent to one.
 */
export function handsBack(e: KeyboardEvent, mac: boolean): boolean {
  if (e.key !== "Enter" || !e.shiftKey || e.altKey) return false;
  return mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** How the rows and the bar spell that chord, on this platform. */
export function handsBackSaid(mac: boolean): string {
  return mac ? "⌘⇧↩" : "Ctrl+Shift+Enter";
}

/**
 * Where on the page a point of the view lands, in the page's CSS pixels: the frame is drawn
 * scaled to fit its box whole, centred (`object-fit: contain`), so a point in the letterbox
 * around it lands on nothing.
 */
export function pagePoint(
  at: { x: number; y: number },
  box: { left: number; top: number; width: number; height: number },
  page: { width: number; height: number },
): { x: number; y: number } | undefined {
  if (box.width <= 0 || box.height <= 0 || page.width <= 0 || page.height <= 0) return undefined;
  const scale = Math.min(box.width / page.width, box.height / page.height);
  const drawnLeft = box.left + (box.width - page.width * scale) / 2;
  const drawnTop = box.top + (box.height - page.height * scale) / 2;
  const x = (at.x - drawnLeft) / scale;
  const y = (at.y - drawnTop) / scale;
  if (x < 0 || y < 0 || x >= page.width || y >= page.height) return undefined;
  return { x: Math.round(x), y: Math.round(y) };
}

const BUTTONS: Record<number, MouseButton> = { 0: "left", 1: "middle", 2: "right" };

/** The mouse button a pointer event is about. */
export function buttonOf(e: { button: number }): MouseButton {
  return BUTTONS[e.button] ?? "none";
}

/** A pointer event of the person's, as the input it sends, aimed at `aim`. */
export function mouseInput(
  aim: string,
  action: "moved" | "pressed" | "released",
  point: { x: number; y: number },
  e: {
    button: number;
    detail: number;
    altKey: boolean;
    ctrlKey: boolean;
    metaKey: boolean;
    shiftKey: boolean;
  },
): BrowserInput {
  return {
    kind: "mouse",
    aim,
    action,
    x: point.x,
    y: point.y,
    button: action === "moved" ? "none" : buttonOf(e),
    clicks: action === "moved" ? 0 : Math.max(1, Math.min(e.detail, 3)),
    modifiers: modifiersOf(e),
    delta_x: 0,
    delta_y: 0,
  };
}

/** A wheel turned over the page, as the input it sends, aimed at `aim`. */
export function wheelInput(
  aim: string,
  point: { x: number; y: number },
  e: {
    deltaX: number;
    deltaY: number;
    altKey: boolean;
    ctrlKey: boolean;
    metaKey: boolean;
    shiftKey: boolean;
  },
): BrowserInput {
  return {
    kind: "mouse",
    aim,
    action: "wheel",
    x: point.x,
    y: point.y,
    button: "none",
    clicks: 0,
    modifiers: modifiersOf(e),
    delta_x: Math.round(e.deltaX),
    delta_y: Math.round(e.deltaY),
  };
}

/** A key of the person's, as the input it sends, aimed at `aim`. */
export function keyInput(
  aim: string,
  down: boolean,
  e: {
    key: string;
    code: string;
    keyCode: number;
    altKey: boolean;
    ctrlKey: boolean;
    metaKey: boolean;
    shiftKey: boolean;
  },
): BrowserInput {
  return {
    kind: "key",
    aim,
    action: down ? "down" : "up",
    key: e.key,
    physical: e.code,
    key_code: e.keyCode,
    modifiers: modifiersOf(e),
  };
}

/**
 * The browsers the person drives now, each with how to hand it a key purlis claimed: the
 * palette's pass-through key, pressed again, reaches the driven browser in focus as it reaches
 * a chat in front.
 */
const driven = new Map<string, (key: string) => void>();

/** Browser `program` is driven by the person, and takes a passed-through key with `send`. Answers
 *  what to call once it is not. */
export function drivenBy(program: string, send: (key: string) => void): () => void {
  driven.set(program, send);
  return () => {
    if (driven.get(program) === send) driven.delete(program);
  };
}

/** Whether the person drives browser `program` now. */
export function isDriven(program: string): boolean {
  return driven.has(program);
}

/** Sends `key` to browser `program`, if the person drives it. Answers whether it was sent. */
export function sendToDriven(program: string, key: string): boolean {
  const send = driven.get(program);
  if (send === undefined) return false;
  send(key);
  return true;
}
