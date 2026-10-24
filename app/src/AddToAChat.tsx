// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { ChatsToPick, type ChatHere, type Referenced } from "./references";
import { picksSaid } from "./referencesMany";
import { Dialog } from "./ui/dialog/Dialog";
import { ListFilter, noMatchSaid, useListFilter } from "./ui/list/ListFilter";

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
  const label = `Add ${picksSaid(referenced)} to a chat's context`;
  // More than about seven chats are filtered (#2910). The keyboard starts on the filter, or on a
  // short list's first chat, as the vault picker's starts on its first row: the dialog's own
  // first field, or else the first thing it can reach.
  const { query, setQuery, shown, filtered } = useListFilter(chats, chatName);
  return (
    <Dialog onClose={onCancel} title={label}>
      <div>
        {filtered && (
          <ListFilter
            noun={CHATS}
            total={chats.length}
            query={query}
            onQuery={setQuery}
            controls="add-to-a-chat-chats"
          />
        )}
        {/* What the filter controls is here whether anything matches or not. */}
        <div id="add-to-a-chat-chats">
          {filtered && shown.length === 0 ? (
            <p className="came-back" role="status">
              {noMatchSaid(CHATS, query)}
            </p>
          ) : (
            <ChatsToPick
              chats={shown}
              label="Chats"
              autoFocus={!filtered}
              onPick={(chat) => {
                onCancel();
                onPick(chat);
              }}
            />
          )}
        </div>
      </div>
    </Dialog>
  );
}

/** What a chat is filtered by: its name. */
const chatName = (chat: ChatHere) => chat.name;

const CHATS = { one: "chat", many: "chats" };
