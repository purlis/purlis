# Amended for #2898, a document focused beside a frame's field, 2026-10-22

What has the focus is the innermost element the engine marks focused, and a focused document gives
way only to an element in its own frame or a frame nested in it (#2294, D-2294-4). The main
document holds every frame, so this rests on how a real engine reports a frame that has lost the
focus. This note records what it reports.

1. **A frame that lost the focus is not reported focused.** With a field in a frame focused, a real
   engine marks the main document, the frame's document and the field focused, and the field is
   what has the focus. Once the outer page is clicked, it marks the main document alone: the
   frame's field, which keeps its place as the frame's last active element, is not marked. So a
   person's key then blanks nothing of the frame, and the field keeps what the agent typed
   (`crates/browser-engine/tests/a_document_focused_beside_a_frames_field_does_not_blank_the_field.rs`,
   Debian's Chromium and the CI runners' Chrome). The rule stands. *D-2898-1, decided in
   implementation, to be confirmed.*
2. **It agrees with D-2295-3** (#2894, for #2295): where the tree still marks several elements,
   the one whose own document says it has the focus (`document.hasFocus()` and its active
   element) is blanked, and where none is named the key is held. Applied after this rule, it holds
   a key in the case this note found does not occur, a frame's stale element marked beside a
   focused outer document. *D-2898-2, decided in implementation, to be confirmed.*
