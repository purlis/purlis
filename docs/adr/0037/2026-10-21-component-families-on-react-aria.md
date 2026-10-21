# Amendment, 2026-10-21 (#2793, #2904): component families on React Aria

The operator's spec #2897 (resolution #2793) replaces this record's rule that **no purlis layer
stands between a screen and a primitive**. Every control is now drawn from a **component
family** in `app/src/ui/<family>`, each wrapping its React Aria primitive exactly once (its
**primitive wrapper**, CONTEXT.md), and a screen never imports the primitives library. The
reasons, the family list and the lint are in
[ADR 0104](../0104-the-window-is-built-from-component-families-on-react-aria-each-primitive-wrapped-once.md).

What this record decided still holds: purlis takes the standard solution for behaviour and keeps
its own visual language, written in tokens. What changes is where the look is attached: to the
family's parts (ADR 0103), not to a class each call site writes.

Radix stays for every family that has not moved yet, and the settings set and the Notice are
drawn from the families as each moves. The button family moved first (#2904): every button,
icon button and button drawn as a link is `Button`, `IconButton` or `ToggleButton`.
