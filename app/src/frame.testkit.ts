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
