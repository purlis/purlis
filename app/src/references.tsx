// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Files into chats** (FM-9, #1103 F7): a file, a folder, a range of lines or a search hit,
 * handed to a chat as a reference in its harness's own syntax, typed and never sent.
 *
 * The window only ever names WHAT to reference — a branch, a path inside it, and the lines —
 * never the text. The core places the path (inside the branch, no link, not `.git`), builds the
 * reference with the chat's harness adapter, and types it as one bracketed paste with nothing
 * after it, only while the chat is at its prompt (`references.rs`). Where it cannot be typed —
 * opencode, a chat mid-turn — the core puts it on the clipboard and says why.
 *
 * Three ways in, all through here:
 * - **a drag** (`dragReference`) from a file row, a search hit or the preview's selection, onto
 *   a chat's tab or its pane (`droppedReference`);
 * - **"Ask a chat about this"** and **"Add to a chat's context"** in the preview, which pick a
 *   chat (`PickAChat`), and **Shift+Enter** on a search hit, which picks one the same way
 *   (`ChatsToPick`, #1151);
 * - **"Start a chat here"** on a file or folder row (`actions.ts`, `startChatHere`).
 */
import { createContext, useContext, useState, type DragEvent } from "react";
import { MessageSquarePlus } from "lucide-react";
import { commands, type Handed, type PlaneId } from "./bindings";
import type { Ran } from "./actions";
import { Button, IconButton } from "./ui/button/Button";
import { InlineMenu, Menu, MenuItem, MenuTrigger } from "./ui/menu/Menu";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** The drag's own type: only charter's rows carry it, so a file dragged in from the desktop or
 *  text from another app is never taken for a reference. */
export const REFERENCE_TYPE = "application/x-charter-reference";

/** What is referenced: a branch by its names, a path inside it, and the lines of a file. */
export type Referenced = {
  /** The project the branch is in: a hit from another project cannot be handed to this one's
   *  chats, whose branches are named in this project. */
  plane: PlaneId;
  workspace: string;
  repo: string;
  piece: string | null;
  path: string;
  folder: boolean;
  lines?: { first: number; last: number };
};

/** How a reference reads in a sentence and a drag's text: `src/main.rs:10-20`. */
export function referenceSaid(r: Referenced): string {
  if (r.lines === undefined) return r.folder ? `${r.path}/` : r.path;
  const { first, last } = r.lines;
  return first === last ? `${r.path}:${first}` : `${r.path}:${first}-${last}`;
}

/** Starts a drag carrying `r`. A copy, never a move: nothing in the branch changes. */
export function dragReference(event: DragEvent, r: Referenced): void {
  event.dataTransfer.setData(REFERENCE_TYPE, JSON.stringify(r));
  event.dataTransfer.setData("text/plain", referenceSaid(r));
  event.dataTransfer.effectAllowed = "copy";
}

/** Whether a drag over a chat carries a reference: what makes the chat a drop target. */
export function carriesReference(event: DragEvent): boolean {
  return Array.from(event.dataTransfer?.types ?? []).includes(REFERENCE_TYPE);
}

/** A drag over a chat: taken as a copy when it carries a reference, and left alone otherwise. */
export function overChat(event: DragEvent): void {
  if (!carriesReference(event)) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = "copy";
}

/** The reference a drop carries, or none for anything else — read whole, and refused unless it
 *  has every field a reference has. */
export function droppedReference(event: DragEvent): Referenced | undefined {
  const raw = event.dataTransfer?.getData(REFERENCE_TYPE);
  if (!raw) return undefined;
  try {
    const r = JSON.parse(raw) as Partial<Referenced>;
    const lines = r.lines;
    const linesOk =
      lines === undefined ||
      (typeof lines === "object" &&
        lines !== null &&
        Number.isInteger(lines.first) &&
        Number.isInteger(lines.last));
    if (
      typeof r.plane !== "string" ||
      typeof r.workspace !== "string" ||
      typeof r.repo !== "string" ||
      (r.piece !== null && typeof r.piece !== "string") ||
      typeof r.path !== "string" ||
      typeof r.folder !== "boolean" ||
      !linesOk
    )
      return undefined;
    return r as Referenced;
  } catch {
    return undefined;
  }
}

/** What the window says once a reference reached a chat, or did not. */
export function handedSaid(handed: Handed, chat: string): string {
  if (handed.kind === "typed") return `Typed ${handed.text} into ${chat}. Nothing was sent.`;
  const why = handed.why.charAt(0).toUpperCase() + handed.why.slice(1);
  return `${why}: ${handed.text}`;
}

/** Hands `r` to chat `session` of project `plane`, named `chat` in what is said. */
export async function handReference(
  plane: PlaneId,
  session: number,
  chat: string,
  r: Referenced,
): Promise<Ran> {
  if (r.plane !== plane)
    return {
      ok: false,
      refused: `${r.path} is in another project, so it was not handed to ${chat}.`,
    };
  const answer = await commands
    .referenceIntoChat(plane, r.workspace, r.repo, r.piece, r.path, r.lines ?? null, session)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (answer.status === "error") return { ok: false, refused: answer.error };
  return { ok: true, said: handedSaid(answer.data, chat) };
}

/** One chat a reference can be handed to, as the picker lists it. */
export type ChatHere = { session: number; name: string };

/** What the project's window lends the views it holds, to hand a reference to a chat. */
export type ChatsForReferences = {
  plane: PlaneId;
  /** Every chat open in the project, in strip order. */
  chats: readonly ChatHere[];
  /** Hand `r` to chat `session`; `ask` also brings the chat to the front. */
  hand: (r: Referenced, session: number, how: "ask" | "add") => void;
  /** Start a chat on `r`'s branch with a reference to it typed as its first prompt, its lines
   *  with it (FM-9, #1151); its answer is said where every row's answer is. */
  start?: (r: Referenced) => void;
};

/** Lent by `PlaneView`; none in a test or a window with no chats to lend. */
export const ReferenceChats = createContext<ChatsForReferences | undefined>(undefined);

/** The chats a view can hand a reference to, where its window lends them. */
export function useReferenceChats(): ChatsForReferences | undefined {
  return useContext(ReferenceChats);
}

/**
 * **"Ask a chat about this" or "Add to a chat's context"**: a button that opens a short list of
 * the project's chats, and hands `referenced` to the one picked. Asking brings the chat to the
 * front, so the operator types their question beside the reference; adding leaves them here.
 */
export function PickAChat({ referenced, how }: { referenced: Referenced; how: "ask" | "add" }) {
  const lent = useReferenceChats();
  const [said, setSaid] = useState(false);
  const label = how === "ask" ? "Ask a chat about this" : "Add to a chat's context";
  if (lent === undefined) return null;
  const chats = lent.chats;
  const button = (
    <Button
      size="sm"
      variant="ghost"
      icon={<MessageSquarePlus className="node-icon" aria-hidden="true" />}
      onPress={chats.length === 0 ? () => setSaid((was) => !was) : undefined}
      aria-expanded={chats.length === 0 ? said : undefined}
    >
      {label}
    </Button>
  );
  // No chat to pick: the button says so in a line under it, and opens no empty menu.
  if (chats.length === 0)
    return (
      <span className="pick-a-chat">
        {button}
        {said && <NoChatToPick />}
      </span>
    );
  return (
    <span className="pick-a-chat">
      <MenuTrigger>
        {button}
        <Menu aria-label={label}>
          {chats.map((chat) => (
            <ChatToPick
              key={chat.session}
              chat={chat}
              onPick={() => lent.hand(referenced, chat.session, how)}
            />
          ))}
        </Menu>
      </MenuTrigger>
    </span>
  );
}

/**
 * **"Start a chat here", from a file's preview** (#1151): the file row's own act (FM-9), with the
 * lines picked in the preview, when there are some, in the reference the new chat is typed. A
 * chat is started on the project's default profile, in the branch's folder; nothing is sent.
 * Drawn only where the window lends a way to start one.
 */
export function StartAChatHere({ referenced }: { referenced: Referenced }) {
  const lent = useReferenceChats();
  const start = lent?.start;
  if (start === undefined) return null;
  return (
    <IconButton
      size="sm"
      variant="ghost"
      aria-label={`Start a chat here on ${referenceSaid(referenced)}`}
      onPress={() => start(referenced)}
      icon={<MessageSquarePlus className="node-icon" aria-hidden="true" />}
    />
  );
}

/** What the pickers say when the project has no chat to hand a reference to. */
const NO_CHAT_OPEN =
  "No chat is open in this project. Start one from the file's row with Start a chat here.";

/** The sentence a picker says where the project has no chat to hand a reference to. */
function NoChatToPick() {
  return (
    <span className="pick-a-chat-none" role="status">
      {NO_CHAT_OPEN}
    </span>
  );
}

/** One chat of a picker, carrying its number: two chats may share a name, and a spec finds the
 *  one it started by this (#1796). */
function ChatToPick({ chat, onPick }: { chat: ChatHere; onPick: () => void }) {
  return (
    <MenuItem id={chat.session} textValue={chat.name} data-session={chat.session} onAction={onPick}>
      {chat.name}
    </MenuItem>
  );
}

/**
 * **The chats to pick from**, as a menu named `label` drawn in place (`InlineMenu`): one row
 * per chat, in the order given, walked by the arrows and chosen by Enter, or the sentence that
 * says none is open. The Search tab draws it under its hits, and Add to a chat in its dialog
 * (#1151).
 */
export function ChatsToPick({
  chats,
  label,
  autoFocus = false,
  onPick,
}: {
  chats: readonly ChatHere[];
  label: string;
  /** Takes the keyboard as it is drawn, on its first chat. */
  autoFocus?: boolean;
  onPick: (chat: ChatHere) => void;
}) {
  if (chats.length === 0) return <NoChatToPick />;
  return (
    <InlineMenu
      className="pick-a-chat-list"
      aria-label={label}
      autoFocus={autoFocus ? "first" : undefined}
    >
      {chats.map((chat) => (
        <ChatToPick key={chat.session} chat={chat} onPick={() => onPick(chat)} />
      ))}
    </InlineMenu>
  );
}

/**
 * The preview's own handle on what it shows: dragged onto a chat, it carries the file — or the
 * lines selected in it — as a reference.
 */
export function DragHandle({ referenced }: { referenced: Referenced }) {
  return (
    <Tooltip tip="Drag onto a chat to type a reference to this into it">
      <span
        className="reference-handle"
        draggable
        role="note"
        data-testid="reference-handle"
        onDragStart={(event) => dragReference(event, referenced)}
      >
        {referenceSaid(referenced)}
      </span>
    </Tooltip>
  );
}
