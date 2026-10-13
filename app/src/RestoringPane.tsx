// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { History } from "lucide-react";

import { EmptyState } from "./EmptyState";

/**
 * A chat's pane before its program has started again: what is happening, in the place its
 * terminal takes. `status`, so a screen reader hears it, and hears the chat arrive in its place.
 */
export function RestoringPane({ name }: { name: string }) {
  return (
    <EmptyState
      mark={History}
      headline={`Restoring ${name}…`}
      body="purlis is starting this chat again. It comes back here as soon as it has started."
      role="status"
      testid="chat-restoring"
    />
  );
}
