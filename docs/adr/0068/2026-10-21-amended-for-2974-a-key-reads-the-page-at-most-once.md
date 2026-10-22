# Amended for #2974, a key reads the page at most once, 2026-10-21

1. **One read of the page for a key, none while its field keeps the focus.** A key the person
   types reads every frame's tree of the page at most once: where it lands (#2638) and which
   field to blank (#2014) are found in that one read, and the field is blanked in a build of the
   same read, not a second one; the key is not sent yet, so the page as read holds what it holds
   then. While the field the last key went to keeps the focus, and neither its frame nor one
   around it has loaded another document since, a key reads the tree not at all, however often
   another frame of the page reloads. *D-2974-1, decided in implementation, to be confirmed.*
