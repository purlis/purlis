// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useRef } from "react";
import * as Dialog from "./overlays/dialog";
import { AnswerBar } from "./AnswerBar";
import { ChatsToPick, type ChatHere, type Referenced } from "./references";
import { picksSaid } from "./referencesMany";
import { Button } from "./ui/button/Button";

/**
 * **"Add to a chat's context" from a file or folder row** (FM-9, #1151): the project's chats,
 * and the one picked has a reference to `referenced` typed in, never sent.
 *
 * The same picker the preview's *Add to a chat's context* and a search hit's Shift+Enter draw
 * (`ChatsToPick`), in a dialog because a row's menu has closed by the time it runs. It types
 * nothing itself: `onPick` hands the reference to the window's one way into a chat
 * (`ChatsForReferences.hand`), whose answer is said where every row's answer is. Several rows
 * picked in the explorer are one paste, and the title says how many (#1151).
 */
export function AddToAChat({
  referenced,
  chats,
  onPick,
  onCancel,
}: {
  referenced: readonly Referenced[];
  chats: readonly ChatHere[];
  onPick: (chat: ChatHere) => void;
  onCancel: () => void;
}) {
  const list = useRef<HTMLDivElement>(null);
  const label = `Add ${picksSaid(referenced)} to a chat's context`;
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="ui-scrim" />
        <Dialog.Content
          className="ui-dialog warning"
          aria-describedby={undefined}
          onOpenAutoFocus={(event) => {
            // The keyboard starts on the first chat, as the vault picker's starts on its first
            // row: the list takes it as it is drawn.
            if (chats.length > 0) event.preventDefault();
          }}
        >
          <Dialog.Title>{label}</Dialog.Title>
          <div ref={list}>
            <ChatsToPick
              chats={chats}
              label="Chats"
              autoFocus
              onPick={(chat) => {
                onCancel();
                onPick(chat);
              }}
            />
          </div>
          <AnswerBar>
            <Button onPress={onCancel}>Cancel</Button>
          </AnswerBar>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
