### Added

- **Open an extension's view beside a pane.** A view's button has a beside icon after it, and an
  ⌥-press (Alt off a Mac), its right-click menu or ⌥Enter on its palette row opens the view in a
  split beside the pane. Where a side would be under about 40 columns by 8 rows, Open beside is
  greyed with its reason before you press. A view already open is brought forward, never opened
  twice (#4769).
- **An extension can offer its view.** An extension that hears an event may offer one of its own
  views. It opens nothing: you get one quiet note with Open, Open beside, Stop offers and Dismiss.
  A dismissed offer stays quiet until purlis restarts. Stopped offers come back from Settings ›
  Extensions. A removed extension's offers go, and a view of it still open says it is not here
  any more (#4769).
