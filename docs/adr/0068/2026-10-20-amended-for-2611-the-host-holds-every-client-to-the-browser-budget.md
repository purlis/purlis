# Amended for #2611, the session host holds every client to the device's browser budget, 2026-10-20

The device's browser budget (#2023, amended 2026-10-18) is the session host's to keep, not only
the window's. This adds to the amendment for #2023; everything else stands as accepted. Each
item is **decided in implementation, to be confirmed** (D-2611-1 to D-2611-4).

1. **The host counts and reserves under its state lock.** At a browser's start the host counts
   every browser it runs that has not ended, whoever started it (the window, another window on
   the same host, a relayed client), and the starts under way, and takes a place for the new one
   in the same step. The place is given back in the step that lists the started browser, or when
   it does not start. Two starts for the last place: one runs, one is refused. *D-2611-1.*
2. **The budget is the host's own device's.** The binary hands the host its browsers
   (`purlis_core::browsers::EngineBrowsers`), which read the budget from this machine's store at
   each start: what its memory gives, or what the person set in Settings › This machine ›
   Browsers. A runner's host reads its own machine's. A host given no way to start browsers has
   no budget. *D-2611-2.*
3. **A start past it is refused `device_full`** (session protocol 1.32, a minor). Its `why`
   names the budget and the setting, whole on its own, since a window before 1.32 shows a code
   it does not know in the host's words. The window maps `device_full` to its device-full
   refusal, which names the setting. *D-2611-3.*
4. **The window keeps its own count first.** It still counts the host's browsers before it
   asks, so its starts stay within the budget on a host before 1.32 that a window came back to
   across an update. The host's answer is the one that holds. *D-2611-4.*
