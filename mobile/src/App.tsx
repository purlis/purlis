import { useCallback, useEffect, useRef, useState } from "react";

import { Chat } from "./Chat.tsx";
// `./home.ts`/`./Home.tsx` and `./lock.ts`/`./Lock.tsx` differ only in case: the extensions
// keep a case-insensitive disk from handing one back for the other.
import { Home } from "./Home.tsx";
import { Lock } from "./Lock.tsx";
import { Pair } from "./Pair.tsx";
import { Settings } from "./Settings.tsx";
import type { Listed } from "./home.ts";
import {
  commands,
  on,
  type Chat as ChatRow,
  type DesktopRow,
  type PhoneInfo,
  type PushedEvent,
  type StandingEvent,
} from "./ipc.ts";
import { atLaunch, hidden, shown, unlocked, type LockState } from "./lock.ts";

type Screen =
  | { at: "home" }
  | { at: "pair" }
  | { at: "settings" }
  | { at: "chat"; device: string; chat: ChatRow };

const LOCK_KEY = "purlis.lock";

/** Whether the lock is on: on by default (iOS design §6.1), a per-phone choice. */
function lockPreferred(): boolean {
  try {
    return localStorage.getItem(LOCK_KEY) !== "off";
  } catch {
    return true;
  }
}

/** A dropped link is dialled again while the app is in front: after two seconds, then twice
 *  as long each time, never longer than this. */
export const REDIAL_FIRST_MS = 2_000;
export const REDIAL_MOST_MS = 30_000;

type Redial = { delay: number; timer: number | null };

/**
 * The shell (iOS design §6): the lock first, then pairing when no desktop is, then Home, a
 * chat or Settings. Coming back to the foreground reconnects every desktop (§6.2) and, after
 * long enough away, locks again. A link that drops while the app is in front is dialled again
 * on its own, with a growing wait, until it holds.
 */
export default function App() {
  const [lockOn, setLockOn] = useState(lockPreferred);
  const [lock, setLock] = useState<LockState>(() =>
    lockPreferred() ? atLaunch() : unlocked(atLaunch()),
  );
  const [phone, setPhone] = useState<PhoneInfo | null>(null);
  const [desktops, setDesktops] = useState<DesktopRow[] | null>(null);
  const [lists, setLists] = useState<Record<string, ChatRow[]>>({});
  const [screen, setScreen] = useState<Screen>({ at: "home" });
  const relisting = useRef<Record<string, number>>({});
  const redials = useRef<Record<string, Redial>>({});

  const relist = useCallback((device: string) => {
    window.clearTimeout(relisting.current[device]);
    relisting.current[device] = window.setTimeout(() => {
      commands
        .list(device)
        .then((chats) => setLists((was) => ({ ...was, [device]: chats })))
        .catch(() => {});
    }, 150);
  }, []);

  const placeDesktop = useCallback((row: DesktopRow) => {
    setDesktops((was) => {
      const rows = was ?? [];
      const at = rows.findIndex((one) => one.device === row.device);
      return at < 0 ? [...rows, row] : rows.map((one, i) => (i === at ? row : one));
    });
  }, []);

  const stopRedial = useCallback((device: string) => {
    const redial = redials.current[device];
    if (redial?.timer !== null && redial?.timer !== undefined) window.clearTimeout(redial.timer);
    delete redials.current[device];
  }, []);

  const redialLater = useCallback(
    (device: string) => {
      if (document.visibilityState === "hidden") return;
      const redial = redials.current[device] ?? { delay: 0, timer: null };
      if (redial.timer !== null) return;
      redial.delay = Math.min(
        redial.delay === 0 ? REDIAL_FIRST_MS : redial.delay * 2,
        REDIAL_MOST_MS,
      );
      redial.timer = window.setTimeout(() => {
        redial.timer = null;
        commands
          .connect(device)
          .then(placeDesktop)
          .catch(() => {});
      }, redial.delay);
      redials.current[device] = redial;
    },
    [placeDesktop],
  );

  const reconnectAll = useCallback(() => {
    commands
      .reconnectAll()
      .then((rows) => {
        setDesktops(rows);
        for (const row of rows) if (row.standing === "connected") relist(row.device);
      })
      .catch(() => {});
  }, [relist]);

  // Once unlocked: who this phone is, who it is paired with, and the links.
  useEffect(() => {
    if (lock.locked) return;
    commands
      .phone()
      .then(setPhone)
      .catch(() => {});
    commands
      .desktops()
      .then((rows) => {
        setDesktops(rows);
        if (rows.length === 0) setScreen({ at: "pair" });
        else reconnectAll();
      })
      .catch(() => setDesktops([]));
  }, [lock.locked, reconnectAll]);

  // The Rust side's word on each desktop's standing, and what the desktops push.
  useEffect(() => {
    const standing = on<StandingEvent>("standing", (event) => {
      setDesktops((was) =>
        (was ?? []).map((row) =>
          row.device === event.device
            ? { ...row, standing: event.standing, why: event.why, generation: event.generation }
            : row,
        ),
      );
      if (event.standing === "connected") {
        stopRedial(event.device);
        relist(event.device);
      } else {
        redialLater(event.device);
      }
    });
    const pushed = on<PushedEvent>("event", (event) => relist(event.device));
    return () => {
      standing.then((off) => off());
      pushed.then((off) => off());
    };
  }, [redialLater, relist, stopRedial]);

  // Background and back (§6.2): the lock's clock, then every link again.
  useEffect(() => {
    const onVisibility = () => {
      const now = Date.now();
      if (document.visibilityState === "hidden") {
        for (const device of Object.keys(redials.current)) stopRedial(device);
        setLock((was) => hidden(was, now));
        return;
      }
      setLock((was) => (lockOn ? shown(was, now) : { ...shown(was, now), locked: false }));
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, [lockOn, stopRedial]);

  // Back in front and not locked: every link again. (Effects run once per change, where an
  // updater may run twice.)
  const wasHidden = useRef(false);
  useEffect(() => {
    if (lock.hiddenAt !== null) {
      wasHidden.current = true;
      return;
    }
    if (wasHidden.current && !lock.locked) {
      wasHidden.current = false;
      reconnectAll();
    }
  }, [lock, reconnectAll]);

  const onUnlocked = useCallback(() => setLock(unlocked), []);

  if (lock.locked) return <Lock onUnlocked={onUnlocked} />;

  const rows = desktops ?? [];
  const retry = (device: string) => {
    stopRedial(device);
    commands
      .connect(device)
      .then(placeDesktop)
      .catch(() => {});
  };

  if (screen.at === "pair" || (desktops !== null && rows.length === 0)) {
    return (
      <Pair
        onBack={rows.length > 0 ? () => setScreen({ at: "home" }) : undefined}
        onPaired={(row) => {
          placeDesktop(row);
          relist(row.device);
          setScreen({ at: "home" });
        }}
      />
    );
  }

  if (screen.at === "chat") {
    const desktop = rows.find((row) => row.device === screen.device);
    if (desktop) {
      return (
        <Chat
          desktop={desktop}
          chat={screen.chat}
          onBack={() => {
            relist(desktop.device);
            setScreen({ at: "home" });
          }}
          onRetry={() => retry(desktop.device)}
        />
      );
    }
  }

  if (screen.at === "settings") {
    return (
      <Settings
        phone={phone}
        desktops={rows}
        lockOn={lockOn}
        onLock={(onNow) => {
          setLockOn(onNow);
          try {
            localStorage.setItem(LOCK_KEY, onNow ? "on" : "off");
          } catch {
            // A phone that keeps nothing keeps the default.
          }
        }}
        onRenamePhone={async (name) => setPhone(await commands.renamePhone(name))}
        onRenameDesktop={async (device, name) =>
          placeDesktop(await commands.renameDesktop(device, name))
        }
        onSetAddresses={async (device, addresses) =>
          placeDesktop(await commands.setAddresses(device, addresses))
        }
        onUnpair={async (device) => {
          await commands.unpair(device);
          stopRedial(device);
          setDesktops((was) => (was ?? []).filter((row) => row.device !== device));
          setLists((was) => {
            const { [device]: _gone, ...kept } = was;
            return kept;
          });
        }}
        onForgetKey={async () => {
          const renewed = await commands.forgetLinkKey();
          for (const device of Object.keys(redials.current)) stopRedial(device);
          setPhone(renewed);
          setDesktops([]);
          setLists({});
          setScreen({ at: "pair" });
        }}
        onBack={() => setScreen({ at: "home" })}
      />
    );
  }

  const listed: Listed[] = rows.map((desktop) => ({
    desktop,
    chats: lists[desktop.device] ?? [],
  }));
  return (
    <Home
      listed={listed}
      onOpen={(device, chat) => setScreen({ at: "chat", device, chat })}
      onRetry={retry}
      onSettings={() => setScreen({ at: "settings" })}
      onPair={() => setScreen({ at: "pair" })}
    />
  );
}
