# Amended 2026-10-20: `theme_preview` names any theme by its id (#2536)

Ruled by the operator on 2026-10-20: `mcp__purlis__theme_preview` **stays pre-allowed** with
the reach below. The 2026-10-17 amendment (#2330) pre-allowed it; #2536 widens what it draws to
any theme the person could draw, and keeps what it may name and what it returns:

- **It returns only pictures of the theme specimen**, the fixed page of sample content drawn in a
  hidden window granted no command. The page is handed the built-in themes, the one theme asked
  for, the colour of the workspace it is tinted with, and the bytes of that theme's own fonts,
  and nothing else of the person's.
- **It names a theme by its id, an appearance (high contrast included, #2709) and option values, and nothing else.** An id is a
  built-in, a theme folder by its name or its level, an extension's theme, or `draft:<folder>`,
  never a path. It takes no workspace, path or command, and it writes, starts and sends nothing.
- **Each id is looked up where the asking chat works**, by the app's own record of that chat:
  the theme folders of its own workspace, the project's and the person's own, the extensions in
  force there, and its own theme drafts. Anything else is refused.
- **A theme is drawn as the window draws it there**: through the theme engine with the consent
  floors held, tinted with that workspace's colour, and in the theme's own fonts, which are read
  only as the project's approval allows. A theme the window would not draw (refused, waiting on
  that approval, or a draft that breaks a hard floor) is not drawn, and the tool says why.
