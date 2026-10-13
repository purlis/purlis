// The commands the Rust side answers and the events it emits (`src-tauri/src/commands.rs`),
// typed once here. Every command carries the desktop's device id, since several may be
// paired, and a chat's id is unique within its desktop.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type PhoneInfo = { id: string; name: string; fingerprint: string };

/** Where a desktop stands: `disconnected` is one not dialled yet. */
export type Standing = "connected" | "reconnecting" | "unreachable" | "disconnected";

export type DesktopRow = {
  device: string;
  name: string;
  addresses: string[];
  standing: Standing;
  why: string | null;
  /** Which link the standing is about: it changes with every connect, so a screen knows to
   *  attach again even when it never saw the link go. */
  generation: number;
};

/** One chat as the desktop lists it (the session protocol's `Chat`). */
export type Chat = {
  chat: string;
  project: string;
  project_path?: string | null;
  title?: string | null;
  /** `working`, `needs_you`, `idle`, `ended`. */
  state: string;
  workspace?: string | null;
  /** 1 a shell tab, 2 a chat its hooks report on, 3 a chat that speaks ACP. */
  level?: number | null;
};

export type Attached = { view: number; cols: number; rows: number };
export type TimelineAttached = { timeline: number; level: number };

export type Choice = { id: string; label: string; lasts: boolean };

/** One item of a chat's timeline (the session protocol's `timeline::Item`, flattened). */
export type Item = { level: number } & (
  | { item: "turn"; turn: string; began: boolean; stop_reason?: string | null }
  | {
      item: "message";
      turn: string;
      role: "user" | "agent" | "unknown";
      text: string;
      done: boolean;
    }
  | {
      item: "tool";
      tool: string;
      title: string;
      kind: string;
      status: string;
      duration_ms?: number | null;
    }
  | { item: "plan"; steps: { text: string; status: string }[] }
  | { item: "ask"; ask: string; summary: string; choices: Choice[]; elicits_secret: boolean }
  | { item: "ask_closed"; ask: string; by: string }
  | { item: "usage"; input_tokens: number; output_tokens: number }
  | { item: "caught_up" }
);

export type ViewEvent = {
  device: string;
  chat: string;
  view: number;
  kind: "snapshot" | "live";
  b64: string;
};
export type StreamEnded = { device: string; chat: string; number: number };
export type TimelineEvent = { device: string; chat: string; timeline: number; item: Item };
export type StandingEvent = {
  device: string;
  standing: Standing;
  why: string | null;
  generation: number;
};
export type PushedEvent = { device: string; pushed: unknown };

export const commands = {
  phone: () => invoke<PhoneInfo>("phone"),
  renamePhone: (name: string) => invoke<PhoneInfo>("rename_phone", { name }),
  desktops: () => invoke<DesktopRow[]>("desktops"),
  pair: (text: string) => invoke<DesktopRow>("pair", { text }),
  renameDesktop: (device: string, name: string) =>
    invoke<DesktopRow>("rename_desktop", { device, name }),
  setAddresses: (device: string, addresses: string[]) =>
    invoke<DesktopRow>("set_addresses", { device, addresses }),
  unpair: (device: string) => invoke<void>("unpair", { device }),
  connect: (device: string) => invoke<DesktopRow>("connect", { device }),
  reconnectAll: () => invoke<DesktopRow[]>("reconnect_all"),
  standing: (device: string) => invoke<DesktopRow>("standing", { device }),
  list: (device: string) => invoke<Chat[]>("list", { device }),
  attach: (device: string, chat: string) => invoke<Attached>("attach", { device, chat }),
  /** The terminal drew `bytes` more of `view`: an ack names its view, so one drawn late for
   *  a view since replaced moves nothing. */
  ack: (device: string, chat: string, view: number, bytes: number) =>
    invoke<void>("ack", { device, chat, view, bytes }),
  detach: (device: string, chat: string) => invoke<void>("detach", { device, chat }),
  attachTimeline: (device: string, chat: string) =>
    invoke<TimelineAttached>("attach_timeline", { device, chat }),
  detachTimeline: (device: string, chat: string) =>
    invoke<void>("detach_timeline", { device, chat }),
  write: (device: string, chat: string, bytes: number[]) =>
    invoke<void>("write", { device, chat, bytes }),
  prompt: (device: string, chat: string, text: string) =>
    invoke<void>("prompt", { device, chat, text }),
  answer: (device: string, chat: string, ask: string, choice: string) =>
    invoke<void>("answer", { device, chat, ask, choice }),
  stop: (device: string, chat: string) => invoke<void>("stop", { device, chat }),
  /** Forget the key and mint the next: the phone, with its new fingerprint. */
  forgetLinkKey: () => invoke<PhoneInfo>("forget_link_key"),
};

/** Listen for one of the Rust side's events; the answer stops listening. */
export function on<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => handler(e.payload));
}

/** What a refusal reads as: the Rust side answers a sentence, or something threw. */
export function said(err: unknown): string {
  return typeof err === "string" ? err : err instanceof Error ? err.message : String(err);
}
