// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Box, CircleHelp, ListChecks, ShieldQuestion, type LucideIcon } from "lucide-react";
import { Badge } from "./ui/badge/Badge";
import type { BlockTagKind } from "./chatLog";

/** The neutral icon beside each tag's word: a line drawing, never an emoji (R-188). */
const ICON: Readonly<Record<BlockTagKind, LucideIcon>> = {
  sandbox: Box,
  harness_check: ListChecks,
  unverified: ShieldQuestion,
  cant_tell: CircleHelp,
};

/**
 * **A block row's source tag** (R-185, R-188; T1-29): its word beside a neutral icon, in the
 * badge family's neutral variant. The icon is hidden from a screen reader, which hears the word.
 */
export function BlockTag({ kind, word }: { kind: BlockTagKind; word: string }) {
  const Icon = ICON[kind];
  return (
    <Badge variant="neutral" className="block-tag">
      <Icon aria-hidden="true" size={12} />
      {word}
    </Badge>
  );
}
