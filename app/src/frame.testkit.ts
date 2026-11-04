// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act } from "@testing-library/react";
import { tellAll } from "./heldStore";

/**
 * **Lets one animation frame go by**, inside `act`, so whatever the window draws once a frame
 * (`heldStore.ts`) is committed before the next line reads it. A test that tells the window
 * about a chat and reads the result at once waits on this in between.
 */
export async function aFramePassed(): Promise<void> {
  await act(() => new Promise<void>((done) => requestAnimationFrame(() => done())));
}

/**
 * **Draws the frame now**, for a test that tells the window about a chat inside `act` and reads
 * the result on the next line: what the chat stores would tell their readers at the next frame,
 * they tell at once. Called inside the same `act` as the event, so it is one commit, as it is in
 * the window.
 */
export function theFrameNow(): void {
  tellAll();
}

/** Where `mockIPC` keeps the window's listeners, by the handler each one was given. */
type Internals = { runCallback: (handler: number, message: unknown) => void };

/**
 * **An event from the core, and the frame the window draws it in**: `message` handed to the
 * listener `mockIPC` gave `handler`, then {@link theFrameNow}, for a test that reads the result on
 * the next line. Called inside `act`, as the event itself would be.
 */
export function heard(handler: number, message: unknown): void {
  (window as unknown as { __TAURI_INTERNALS__: Internals }).__TAURI_INTERNALS__.runCallback(
    handler,
    message,
  );
  theFrameNow();
}
