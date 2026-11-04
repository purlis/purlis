// Probe for #4098, never merged: what lets the Chats list's hold go.

type Entry = Record<string, unknown>;

const g = globalThis as unknown as { __heldLog?: Entry[] };

export function heldLog(): Entry[] {
  g.__heldLog ??= [];
  return g.__heldLog;
}

function describe(target: EventTarget | null | undefined): string {
  if (target === null || target === undefined) return "null";
  if (target === window) return "window";
  if (target === document) return "document";
  if (!(target instanceof Element)) return String(target);
  const cls = typeof target.className === "string" ? target.className.split(" ")[0] : "";
  const s = target.getAttribute("data-session");
  return `${target.tagName.toLowerCase()}${cls ? "." + cls : ""}${s ? "#" + s : ""}`;
}

export function note(why: string, event?: Event | null, more: Entry = {}): void {
  const log = heldLog();
  if (log.length > 400) log.splice(0, 100);
  const e = event as (Event & { relatedTarget?: EventTarget | null }) | null | undefined;
  log.push({
    t: Math.round(performance.now()),
    why,
    ...(e
      ? {
          type: e.type,
          trusted: e.isTrusted,
          target: describe(e.target),
          related: "relatedTarget" in e ? describe(e.relatedTarget) : undefined,
        }
      : {}),
    ...more,
  });
}

/** Every pointer, mouse and focus event the window hears, while on. */
export function recordEvents(): () => void {
  const types = [
    "pointerover",
    "pointerout",
    "pointerenter",
    "pointerleave",
    "pointermove",
    "pointerdown",
    "pointerup",
    "pointercancel",
    "mouseover",
    "mouseout",
    "focusin",
    "focusout",
    "blur",
    "focus",
    "visibilitychange",
  ];
  const hear = (event: Event) => note("event", event);
  for (const type of types) window.addEventListener(type, hear, true);
  return () => {
    for (const type of types) window.removeEventListener(type, hear, true);
  };
}
