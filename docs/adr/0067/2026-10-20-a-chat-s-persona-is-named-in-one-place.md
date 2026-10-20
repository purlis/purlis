# Amended (2026-10-20, #2691): a chat's persona is named in one place, and project-wide browser sites count only under the sandbox

This adds to the amendments *a browser site can be allowed for everyone in the project* (#2443)
and *a chat without the sandbox gets its persona's browser logins* (#2560). Everything else
stands. Both items are **decided in implementation, to be confirmed** (D-2691-1, D-2691-2).

1. **A browser site committed for everyone in the project counts only where the project's
   sandbox is in force.** In a project that has not turned it on, a committed site reaches no
   browser, even one recorded as seen on this machine, and the Notice of the project's browser
   sites tells nothing there: a site seen here is not told as taken out, and stays seen for when
   the sandbox is on again. The window offers *Allow for everyone in
   this project* only where it counts, to a chat's browser and to one the person opened, and an
   Allow at that scope is refused in words elsewhere. In a project without the sandbox, a
   browser site reaches a browser by an Allow the person makes on this machine.
   *D-2691-1.*
2. **One function names a chat's persona for every path** (`Chats::persona_of`), read from the
   app's record of the chat's run. It names two roles, never chosen apart by a path:
   - **the persona it was started as**, whose vaults it may use. A browser login's vault is
     judged by it, as `secret exec` judges it. A chat on no persona uses no persona's vault.
   - **the persona whose grants it runs with**: for a chat on a profile, the grants a handoff
     left it holding, else its own, else the project's default persona; a chat on no profile
     runs with no persona's grants. It is read at the chat's start, the persona its
     sandbox was compiled for, and is what its browser reaches and what its Asks are about. It
     is fixed at the start, as the sandbox is. An Allow of its own grants, or a new default,
     changes it at the next start.

   *D-2691-2.*
