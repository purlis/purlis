// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { rowKey } from "./keys/registry";
import { MenuShortcuts } from "./ui/menu/Menu";
import { attach } from "./bench";
import { settleLayout } from "./regions";
import { keepTheirOrder } from "./commandOrder";
import { startTheSwitchClock } from "./switchClock";
import { watchTheAnswers } from "./answerWatch";
import { drawWindowText, listenForSizeKeys, onTextSizes, textSizes } from "./textSize";
import {
  DEFAULT_THEME,
  drawIn,
  fetchFacesWith,
  holdFacesForTheFirstFrame,
  personInForce,
} from "./theme/theme";
import { sayAppearance } from "./appearance";
import { fetchFace } from "./theme/faces";
import { firstFollowsSystem, firstFrameOnce } from "./windowprefs";
import { drawNatively } from "./theme/native";

// The window's colours, before anything is rendered and therefore before anything is painted:
// the theme the levels choose for the project and workspace the window was created on (#2602),
// resolved once by the core's theme engine before the window existed and handed to it with the
// window (`windowprefs.ts`), so nothing is read or asked on the way to the first frame — the only
// arrangement ADR 0026's 2 s cold start can afford. The native window was created in the same
// answer, and the window's later ask of what is in force answers it again: nothing is repainted,
// unless a theme's parts were still being measured when the window was created (#4337): they
// land with that answer.
// A theme's fonts are asked for by digest after the frame (`theme/faces.ts`, #2326): the first
// frame is drawn in each family list's fallback, and nothing waits for a face.
fetchFacesWith(fetchFace);
// Its faces wait for the first frame to be on screen (`App.tsx` lets them go), so decoding them
// is never on the way to it (ADR 0026, #3034).
holdFacesForTheFirstFrame();
const first = firstFrameOnce()?.theme ?? DEFAULT_THEME;
drawIn(first);
// The window was created in this theme's native appearance and colour (`nativewindow.rs`);
// it is told again, for what only the page knows: the system's appearance a theme that follows
// it is drawn in, and `prefers-contrast` where the core read no contrast (#2328, #2582).
drawNatively(first, firstFollowsSystem());
// The person's density and fonts were drawn with it, as the engine read them from the layout
// file (`appearance.ts`); what it had to put right is said.
sayAppearance(personInForce().complaints);

// The window's text size, for the same reason and from the same place: the layout file the
// window was handed (`textSize.ts`, charter-app#283). The root's font size is what every `rem`
// in the stylesheet is measured by, so the whole window is drawn at it from the first frame,
// and again whenever it changes. The size keys — ⌘/Ctrl with =, - and 0 — are listened for on
// the window from here on, so they work before any project is open.
drawWindowText(textSizes().window);
onTextSizes((sizes) => drawWindowText(sizes.window));
listenForSizeKeys(window);

// In a scenario-test build only, WebdriverIO's window-side plugin, which its Tauri service
// looks for. `vite build --mode e2e` is the only build that sets this.
if (import.meta.env.VITE_E2E) void import("@wdio/tauri-plugin");

// The seam `tools/bench.mjs` measures the window through, in that same build and no other.
attach();

// The core's answers that read and change the same thing, in the order they were asked
// (`commandOrder.ts`): the core answers them off the window's thread.
keepTheirOrder();

// What each project switch took in this window, for the doctor's dialog (`switchClock.ts`).
startTheSwitchClock();

// A command the core has not answered for a while is said on the window's line, and a failed
// scenario test reads which (`answerWatch.ts`).
watchTheAnswers();

// DEBUG-4351: names every setState while an opening is timed.
{
  const internals = (React as unknown as Record<string, any>)
    .__CLIENT_INTERNALS_DO_NOT_USE_OR_WARN_USERS_THEY_CANNOT_UPGRADE;
  const g = globalThis as unknown as { __calls?: string[]; __t0?: number };
  const said = (kind: string) => {
    const frames = (new Error().stack ?? "").split("\n").slice(2, 5);
    const short = frames.map((f) => f.replace(/[a-z]+:\/\/[^ ]*\/assets\/[^:]*/g, "")).join(" < ");
    g.__calls?.push(`${kind} +${Math.round(performance.now() - (g.__t0 ?? 0))} ${short}`);
  };
  const stable = new WeakMap<object, unknown>();
  const named = (setter: (...a: unknown[]) => void) => {
    let one = stable.get(setter);
    if (one === undefined) {
      one = (...a: unknown[]) => {
        said("SET");
        setter(...a);
      };
      stable.set(setter, one);
    }
    return one;
  };
  let current: any = internals.H;
  const wrapped = new WeakSet<object>();
  Object.defineProperty(internals, "H", {
    get: () => current,
    set: (d: any) => {
      if (d && !wrapped.has(d)) {
        wrapped.add(d);
        for (const hook of ["useState", "useReducer"]) {
          const real = d[hook];
          if (typeof real !== "function") continue;
          d[hook] = (...a: unknown[]) => {
            const answer = real.apply(d, a);
            return [answer[0], named(answer[1])];
          };
        }
      }
      current = d;
    },
  });
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {/* Every menu's row says the key the key registry holds for its act (#2921, N25). */}
    <MenuShortcuts.Provider value={rowKey}>
      <React.Profiler
        id="App"
        onRender={(_id, phase, actual, base, start, commit) => {
          // DEBUG-4351
          const g = globalThis as unknown as { __calls?: string[]; __t0?: number };
          g.__calls?.push(
            `COMMIT ${phase} react=${Math.round(actual)} at+${Math.round(start - (g.__t0 ?? 0))} base=${Math.round(base)} done+${Math.round(commit - (g.__t0 ?? 0))}`,
          );
        }}
      >
        <App />
      </React.Profiler>
    </MenuShortcuts.Provider>
  </React.StrictMode>,
);

// The theme an approved extension contributes, if one does, is drawn by `App` once it knows which
// project is in front — that project decides whose (ADR 0048) — and so after the render above,
// never before it: it is a disk read and a fingerprint of every file each installed extension
// declares (ADR 0041's named cost), which must not sit between the process starting and the
// first frame.

// What the layout file cost, said in the Inbox, and the one-time move of the arrangement
// web storage used to hold into the file. After the render for the same reason: the first frame
// was already drawn from what the window was handed.
void settleLayout();
