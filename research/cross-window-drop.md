# Dropping a pane into another window's zones (#4706)

Researched 2026-11-02 against purlis `origin/main` 6b8cee297 (`app/`).
Tags: **[V]** means verified from a primary source, with a citation. **[I]** means inferred from sources. Nobody has tested it on hardware.

## Question

Can a pane or tab be dropped into **another** window's panes, using the edge and centre zones of #4583? An outer quarter by an edge splits on that side. The middle swaps. The target pane shows the result before the drop.

1. Can Tauri carry an HTML drag from one webview to another on macOS (WKWebView), Windows (WebView2) and Linux (WebKitGTK)?
2. Today's drags use @dnd-kit, which runs on pointer events. What happens to those when the pointer leaves the window?
3. If native drag fails, what would a drag brokered by the core cost?
4. Can keyboard placement mode aim at another window's panes?

## Short answer

- Native HTML5 drag and drop **can** cross windows. All three engines are OS-level drag sessions, and a custom MIME type survives between same-origin pages. Every purlis window has the same origin. [V/I]
- But Tauri's default `dragDropEnabled: true` **blocks HTML5 drops on Windows and on macOS** in the Tauri version we ship (2.12.5 / runtime-wry 2.12.3). A macOS fix is open, not merged. Linux is not blocked by Tauri, but WebKitGTK's HTML drag and drop has open bugs. [V]
- @dnd-kit pointer drags can never drop into another webview. The source webview keeps the pointer until release, and the other window gets no events at all. [V/I]
- A drag brokered by the core works on macOS, Windows and X11. It is blind on **Wayland**, where there is no global cursor or window position. Tauri has no z-order or "window under point" API, so overlapping windows need app-side tracking. [V]
- Placement mode across windows is feasible if the core holds the "picked up" state. It needs focus to move between windows, which is weak on Wayland. [I]
- **Recommendation:** keep cross-window zone drops **out** of this spec. Ship menu rows and tear-off (#4701). Keep placement mode in-window. File a follow-up spike for an HTML5-DnD cross-window leg. Separately, check the existing reference drag (`references.tsx`): it is likely broken today on macOS and Windows for the same reason.

## Findings per platform

### What Tauri's `dragDropEnabled` does

- It is a `WindowConfig` field, default `true`. Its doc says it enables "the drag and drop handlers used internally to generate `DragDropEvent`s", and that disabling it "is required to use HTML5 drag and drop on the frontend on Windows". It maps to `WebviewBuilder::disable_drag_drop_handler`. [V] [1]
- In tauri-runtime-wry, the handler Tauri installs **always returns `true`**, which means "block the OS default". [V] [2]
- Purlis does not set `dragDropEnabled` in `tauri.conf.json`. Split windows clone the main window's config (`windows.rs` `make_split`, `WebviewWindowBuilder::from_config`), so every window runs with the handler on. [V, local]
- Purlis never listens for Tauri's `DragDropEvent` (grep of `app/src` and `app/src-tauri/src`). Turning the handler off loses nothing it uses today. [V, local]

### Windows (WebView2)

- When a handler is set, wry calls `SetAllowExternalDrop(false)` and replaces the webview's OLE drop target with `RevokeDragDrop` + `RegisterDragDrop`. [V] [3][4]
- wry's docs say this disables HTML drag and drop (`draggable="true"`), and that the handler's return value is ignored on Windows. [V] [5]
- A Tauri maintainer says no hybrid mode is possible: WebView2 will not let Tauri forward its Win32 drop events. [V] [6]
- WebView2's own spec lists "Drag&Drop hyperlinks from one webview2 to another webview2" as an **external** drop that `AllowExternalDrop` governs. A drop from one purlis window into another is therefore "external" to WebView2, and with Tauri's default it is refused. [V] [7] WebView2 staff confirm that `AllowExternalDrop = false` blocks drags inside the page too. [V] [8]
- With `dragDropEnabled: false`, WebView2 keeps its default drop target, and Chromium carries custom MIME types between windows. VS Code relies on this: it drags editors between windows as a `CodeEditors` JSON type. [I from V] [17][18]

### macOS (WKWebView)

- wry overrides `draggingEntered:`, `draggingUpdated:`, `performDragOperation:` and `draggingExited:` on its WKWebView subclass. When the handler returns `true`, wry never calls `super`, so WebKit never sees the drag. [V] [9][10]
- Because Tauri's handler always returns `true`, HTML5 drops are blocked on macOS too, whatever the config doc says. Tauri issue #11930 and docs issue #14373 report this. A maintainer confirmed it in wry PR #1764: "it works on macOS … In Tauri we always return `true`". [V] [11][12][13]
- The fix, tauri PR #16373 ("stop blocking default drag drop", return `false`), is **open** and targets tauri 2.12.6. Purlis is on tauri 2.12.5 and tauri-runtime-wry 2.12.3 (Cargo.lock). [V] [14]
- Custom types between two WKWebViews: WebKit tags custom pasteboard data with the page's origin string. It returns that data only when the reading page has the same origin (`DataTransfer::readStringFromPasteboard`, `Document::originIdentifierForPasteboard`). Custom pasteboard data is on by default on Cocoa, GTK, WPE and Win. All purlis windows load the same app origin, so a custom `application/x-purlis-pane` type should survive. [V for the mechanism, I for purlis] [15][16][19]
- The OS drives the drag session (NSDraggingDestination), so another window of the same app gets `dragover` and `drop` once WebKit is allowed to see the drag. [I]

### Linux (WebKitGTK)

- wry connects GTK `drag-data-received`, `drag-motion`, `drag-drop` and `drag-leave`. It only acts on URI-list drags (`info == 2`, files) and returns `false` otherwise, so the default handling goes on. HTML drags are not blocked by Tauri on Linux. A maintainer confirmed this in wry PR #1764: "Linux's HTML drag and drop still works with `with_drag_drop_handler` set". [V] [20][13]
- WebKitGTK's own HTML drag and drop is unreliable. Tauri #6695 (open since 2023) reproduces it in Epiphany too, and #12052 (macOS works, Linux fails, with `dragDropEnabled: false`) was closed as its duplicate. [V] [21][22]
- On Wayland, native drag and drop is the only cross-window route the compositor supports. Chromium's Wayland tab dragging uses the Wayland DnD protocol "to keep track of cursor location and surface focus". [V] [23]

### Pointer drags (@dnd-kit) leaving the window

- @dnd-kit 6.3.1's pointer sensor puts move/up listeners on the pressed element (or its document). It does not call `setPointerCapture`. It **cancels** on window `resize` and `visibilitychange`. [V] [24]
- **macOS:** "Mouse-dragged and mouse-up events are typically sent to the same view that received the most recent mouse-down event." So the source WKWebView keeps getting `pointermove` (coordinates outside its bounds) and the final `pointerup`, even outside the window. A mouse-down in a non-key window is discarded unless `acceptsFirstMouse` is set. [V] [25]
- **Windows:** windowed WebView2 keeps a pointer-captured gesture alive outside the page. A Tauri app's custom window resize built on `setPointerCapture` + `pointermove` works with WebView2's defaults. `AllowHostInputProcessing = true` breaks it and can lose the mouse release. [V] [26] The general behaviour (Chromium uses Win32 mouse capture during a press) is [I]. learn.microsoft.com was refused, so the `SetCapture` doc is not cited.
- **GTK/X11/Wayland:** a button press gives the pressed surface an implicit grab until release, so the source webview keeps getting motion and the release. [I]. docs.gtk.org and wayland.freedesktop.org were refused. Chromium's Wayland drag controller tracks "the window that holds the pointer grab … that must receive the mouse release event". [V] [23]
- Pointer Events spec: `setPointerCapture` retargets a pointer's events to one element, and capture is released right after `pointerup`/`pointercancel`. It says nothing about events outside the window. That part is up to the platform. [V] [27]
- **Upshot:** a pointer drag stays owned by the source webview from press to release. The target window gets no `pointermove`, `pointerover` or `drop`. Only something outside both webviews (the core) can tell the target where the pointer is. [I from V]

## Core-brokered drag

### Design sketch

1. A drag starts in window A (dnd-kit, as today). When the pointer leaves A's bounds, A sends `drag_started { item }` to the core (a command).
2. The core polls `AppHandle::cursor_position()` (desktop coordinates, physical) at about 60 Hz. This exists in Tauri 2 [V] [28]. It avoids using the webview's `screenX`, which is wrong on macOS WKWebView (Tauri #14673: `screenLeft` stuck at fixed values). [V] [29]
3. The core hit-tests every purlis window with `inner_position()` + `inner_size()` (physical) [V] [30]. It picks the topmost one from a focus-order stack the app keeps itself (`WindowEvent::Focused`). Tauri and tao have no z-order or "window under point" API: a grep of `tauri/src/window/mod.rs` and `tao/src/window.rs` finds only z-order notes about owned windows. [V] [30][31] Chromium needs `GetLocalProcessWindowAtPoint(point, ignore)` for exactly this, on top of native window enumeration. [V] [32]
4. The core emits `drag_over { x, y }` to window B in B's **logical client** coordinates: `(cursor − inner_position) / scale_factor(B)`. B's overlay hit-tests its panes and draws the zones and the preview.
5. On release, A's `pointerup` (outside A) sends `drag_ended` to the core. The core sends `drop` to B, and B answers with the zone. The core moves the pane (the same path the menu rows use) and tells A and B to re-render.

### Cost

- **Rust (core):** a drag broker module with state, polling, hit test, focus-order tracking, and coordinate conversion with per-monitor scale. About 300–500 lines with tests. [I]
- **TS (frontend):** a source-side hook (detect the pointer leaving, forward the drag, cancel on Esc) and a target-side "remote drag" overlay that reuses the #4583 zone math. About 300–400 lines. dnd-kit's cancel-on-resize/visibility must be kept in mind. [I]
- **Latency:** Tauri publishes no per-frame IPC numbers. Its docs say the event system "is not designed for low latency or high throughput" and point to channels for streaming. [V] [33] Each frame is a JS→Rust invoke plus a Rust→JS `eval` (`emit_js`). [V] [34] Expect a few ms per hop; measure in a spike. [I]
- **DPI and multi-monitor:** positions are physical. macOS has a known bug where secondary monitors' physical positions are computed from the primary's *scaled* size (Tauri #7890, open). Mixed-DPI setups would mis-aim. [V] [35]
- **Wayland:** `outer_position` "has no effect, since Wayland doesn't support a global coordinate system". `cursor_position` is "Unsupported, returns `0,0`". The broker cannot work there at all. [V] [31]
- **Overlapping windows / target behind the source:** without z-order the core must guess from its own focus stack. A target fully hidden behind A cannot be reached unless the core raises it (for example on a dwell) or A is shrunk or moved away. Another app's window on top would be ignored by our hit test (Chromium's API returns null in that case; ours would not know). [I from V] [32]
- **macOS first-click:** a drag that starts in a non-key window loses its mouse-down unless `acceptsFirstMouse` is on. That already applies to today's in-window drags. [V] [25]

## Placement mode across windows

- Mechanism: the core holds `picked_up { item, from_window }` and broadcasts it. Every window draws the five targets on its panes. Arrows and Tab move within the focused window. A "next window" key (or Tab past the last pane) makes the core call `set_focus` on the next window ("Bring the window to front and focus"). [V for `set_focus`] [30][31] Enter drops through the same core move. Esc cancels everywhere.
- macOS: focusing another window of the same, already-active app is allowed. [I]
- Wayland: activation needs a token from the compositor. GTK may only flag the window as wanting attention, so the focus jump can fail. [I] wayland.freedesktop.org was refused, so no citation.
- Screen readers: moving focus across windows is announced as a window change (VoiceOver, NVDA), and the user loses their place. Each window needs a live-region line ("Placing chat X: window 2 of 3, pane Y, split left") so the state is heard in both windows. [I]
- WCAG 2.5.7 only needs a single-pointer alternative to dragging. The menu rows ruled in #4701 (move to window…, then pick a pane and side) already meet it across windows. Placement mode across windows is a nice-to-have, not a compliance need. [I]

## Prior art: VS Code

- 1.85 (November 2023) added floating editor windows: "drag an editor out of the current window and drop it on an empty space on your desktop", plus "Move Editor into New Window" commands. [V] [36]
- Auxiliary windows are opened with `mainWindow.open('about:blank', …)` from the main renderer, so they **share one JS context**. [V] [37]
- Editors cross windows with **HTML5 drag and drop**: `dataTransfer.setData(CodeDataTransfers.EDITORS /* 'CodeEditors' */, …)`, "enables cross window DND of editors". [V] [17][18] Ctrl-drag "copy a tab into another VS Code window" works across separate main windows too (issue #283423, a regression report). [V] [38]
- Groups and in-window drags use an in-memory `LocalSelectionTransfer` singleton, which only works in one JS context. [V] [39] A `BroadcastDataChannel` tells all windows "something is being dragged over". [V] [18]
- The edge zones are 10% of the width/height for editors and 10–30% for groups (`editorDropTarget.ts`). [V] [40]
- Chromium tab dragging between browser windows is native, not HTML. It finds the target with `GetLocalProcessWindowAtPoint` (ignoring the dragged window). On Wayland it rides the DnD protocol. [V] [32][23]
- Lesson: VS Code got cross-window drops cheaply because Chromium's HTML5 DnD crosses windows and nothing on top blocks it. Purlis would need the same native route. A pointer drag cannot get there. [I]

## Recommendation for the spec

1. **Keep cross-window zone drops out of #4583's spec.** Menu rows plus tear-off (#4701) cover moving between windows. Placement mode stays inside one window.
2. **Do not build the core-brokered drag.** It is dead on Wayland, has no z-order source, inherits macOS multi-monitor coordinate bugs, and costs about 600–900 lines for a gesture a menu row already covers.
3. **File a later ticket, "Cross-window drop over native drag and drop"**, gated on a spike:
   - Set `dragDropEnabled: false` on the main config (split windows inherit it). Purlis uses no Tauri `DragDropEvent`. Or wait for tauri #16373 on macOS; Windows needs `false` regardless.
   - Prototype: a tab or pane handle that starts an HTML5 drag (`draggable`, `application/x-purlis-pane` carrying only ids) once the pointer nears the window edge, or always. Window B's overlay listens for `dragover`/`drop` and reuses the zone math. The core does the move.
   - Test on macOS, Windows, X11 and Wayland, including WebKitGTK (#6695) and xterm panes under the overlay.
   - Open question for the spike: dnd-kit cannot hand a live pointer drag to a native drag mid-gesture. HTML drags only start from `dragstart`. So the pane handle would need native drag for the whole gesture, or a mode switch. [I]
4. **Check the reference drag now (separate bug).** `app/src/references.tsx` uses HTML5 `dataTransfer` (`application/x-charter-reference`) while `dragDropEnabled` is the default. By the sources above, its drops into chats are likely refused today on macOS and Windows. Check this on the shipped app before the spike. If confirmed, the fix is `dragDropEnabled: false`. [I]

## Sources

1. Tauri `WindowConfig.drag_drop_enabled` doc: https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-utils/src/config.rs (search `drag_drop_enabled`)
2. tauri-runtime-wry drag-drop handler returning `true`: https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-runtime-wry/src/lib.rs#L4786-L4817
3. wry WebView2 `SetAllowExternalDrop(false)`: https://github.com/tauri-apps/wry/blob/dev/src/webview2/mod.rs (search `SetAllowExternalDrop`)
4. wry WebView2 `RevokeDragDrop`/`RegisterDragDrop`: https://github.com/tauri-apps/wry/blob/dev/src/webview2/drag_drop.rs
5. wry `with_drag_drop_handler` doc: https://github.com/tauri-apps/wry/blob/dev/src/lib.rs (search `with_drag_drop_handler`)
6. Tauri #15138 (Windows: no hybrid mode): https://github.com/tauri-apps/tauri/issues/15138
7. WebView2 AllowExternalDrop API spec: https://github.com/MicrosoftEdge/WebView2Feedback/blob/main/specs/APIReview_AllowExternalDrop.md
8. WebView2Feedback #4830: https://github.com/MicrosoftEdge/WebView2Feedback/issues/4830
9. wry macOS NSDraggingDestination overrides: https://github.com/tauri-apps/wry/blob/dev/src/wkwebview/class/wry_web_view.rs
10. wry macOS drag handlers (skip `super` when handler is true): https://github.com/tauri-apps/wry/blob/dev/src/wkwebview/drag_drop.rs
11. Tauri #11930: https://github.com/tauri-apps/tauri/issues/11930
12. Tauri #14373: https://github.com/tauri-apps/tauri/issues/14373
13. wry PR #1764 and comments: https://github.com/tauri-apps/wry/pull/1764
14. Tauri PR #16373 (open): https://github.com/tauri-apps/tauri/pull/16373
15. WebKit `DataTransfer.cpp` (same-origin custom data): https://github.com/WebKit/WebKit/blob/main/Source/WebCore/dom/DataTransfer.cpp
16. WebKit `Document::originIdentifierForPasteboard`: https://github.com/WebKit/WebKit/blob/main/Source/WebCore/dom/Document.cpp
17. VS Code `CodeDataTransfers.EDITORS = 'CodeEditors'`: https://github.com/microsoft/vscode/blob/main/src/vs/platform/dnd/browser/dnd.ts
18. VS Code workbench dnd ("enables cross window DND of editors", `GlobalWindowDraggedOverTracker`): https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/dnd.ts
19. WebKit `CustomPasteboardDataEnabled` defaults: https://github.com/WebKit/WebKit/blob/main/Source/WTF/Scripts/Preferences/UnifiedWebPreferences.yaml
20. wry WebKitGTK drag handlers: https://github.com/tauri-apps/wry/blob/dev/src/webkitgtk/drag_drop.rs
21. Tauri #6695 (WebKitGTK DnD): https://github.com/tauri-apps/tauri/issues/6695
22. Tauri #12052: https://github.com/tauri-apps/tauri/issues/12052
23. Chromium Wayland window drag controller: https://chromium.googlesource.com/chromium/src/+/main/ui/ozone/platform/wayland/host/wayland_window_drag_controller.h
24. @dnd-kit/core 6.3.1 AbstractPointerSensor: https://github.com/clauderic/dnd-kit/blob/%40dnd-kit/core%406.3.1/packages/core/src/sensors/pointer/AbstractPointerSensor.ts
25. Apple, Cocoa Event Architecture (mouse-dragged/up go to the mouse-down view; non-key window discards mouse-down): https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/EventArchitecture/EventArchitecture.html
26. WebView2Feedback #5701 (Tauri pointer-captured resize): https://github.com/MicrosoftEdge/WebView2Feedback/issues/5701
27. W3C Pointer Events Level 3, §9 Pointer capture: https://www.w3.org/TR/pointerevents3/
28. Tauri `AppHandle::cursor_position` / `monitor_from_point`: https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/app.rs (search `cursor_position`); https://docs.rs/tauri/latest/tauri/struct.AppHandle.html
29. Tauri #14673 (window position / `screenLeft` wrong on macOS): https://github.com/tauri-apps/tauri/issues/14673
30. Tauri `Window` (`inner_position`, `outer_position`, `scale_factor`, `set_focus`, z-order notes): https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/window/mod.rs
31. tao `Window` platform notes (Wayland position and cursor unsupported; `set_focus`): https://github.com/tauri-apps/tao/blob/dev/src/window.rs
32. Chromium `WindowFinder` / `Screen::GetLocalProcessWindowAtPoint`: https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/ui/views/tabs/dragging/window_finder.h ; https://chromium.googlesource.com/chromium/src/+/main/ui/display/screen.h
33. Tauri, Calling the frontend from Rust (event system not for low latency): https://v2.tauri.app/develop/calling-frontend/
34. Tauri event emit path (`plugin:event|emit`, `emit_js` → `eval`): https://github.com/tauri-apps/tauri/blob/dev/packages/api/src/event.ts ; https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/webview/mod.rs
35. Tauri #7890 (macOS physical positions wrong across monitors): https://github.com/tauri-apps/tauri/issues/7890
36. VS Code 1.85 release notes, Floating editor windows: https://code.visualstudio.com/updates/v1_85
37. VS Code auxiliary window service (`mainWindow.open('about:blank')`): https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/auxiliaryWindow/browser/auxiliaryWindowService.ts
38. VS Code #283423 (Ctrl-drag tab between windows): https://github.com/microsoft/vscode/issues/283423
39. VS Code `LocalSelectionTransfer` singleton: https://github.com/microsoft/vscode/blob/main/src/vs/platform/dnd/browser/dnd.ts (search `LocalSelectionTransfer`)
40. VS Code editor drop target thresholds: https://github.com/microsoft/vscode/blob/main/src/vs/workbench/browser/parts/editor/editorDropTarget.ts
41. Tauri #9976 (drag content between webviews; maintainer: webviews don't share a JS context): https://github.com/tauri-apps/tauri/issues/9976

Hosts refused by the network sandbox: learn.microsoft.com, html.spec.whatwg.org, webkit.org, developer.mozilla.org, docs.gtk.org, wayland.freedesktop.org, gitlab.freedesktop.org, gitlab.gnome.org, source.chromium.org, electronjs.org, w3c.github.io. Claims that would have cited them are marked [I].
