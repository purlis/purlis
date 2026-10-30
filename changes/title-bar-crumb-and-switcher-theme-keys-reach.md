### Fixed

- **A theme's title-bar crumb, ⌄, switcher rows and pins now draw** (#3656). A theme's
  `titlebar.crumb.*`, `titlebar.all.*`, `titlebar.row.*` and `titlebar.pin.*` keys were accepted
  but reached nothing, because they were written as parts inside a workspace chip. Each now
  reaches its own element, at rest and in the states it is marked in: under the pointer, open
  or pinned, and the keyboard's focus. A key that names one of them in a chip's variant
  (`titlebar.crumb.pinned.fill`), or in a state it is never in (`titlebar.row.focused.fill`),
  reaches nothing: the schema no longer offers it, and a theme that writes it is told so. Such a
  key that also set a size inside a state, or had a value of the wrong kind, used to refuse the
  whole theme; it is now only left out.
