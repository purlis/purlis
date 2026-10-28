// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { KeyboardEvent } from "react";
import { isKey } from "./keys/registry";
import { moveAlong } from "./tabSequence";

/**
 * **How the keyboard leaves a surface that keeps every other key: Ctrl+Tab forwards,
 * Ctrl+Shift+Tab backwards** (#2028): a browser's page while the person drives it, and a
 * browser's DevTools, as a chat's terminal does (`SessionPane.leavesTheChat`, the same chord,
 * `chat.leave`). On the capture phase of the surface, before its own key handling, so the
 * chord never reaches the page or DevTools.
 */
export function leavesTheSurface(event: KeyboardEvent<HTMLElement>) {
  if (!isKey(event.nativeEvent, "chat.leave")) return;
  event.preventDefault();
  event.stopPropagation();
  moveAlong(event.currentTarget, event.shiftKey);
}
