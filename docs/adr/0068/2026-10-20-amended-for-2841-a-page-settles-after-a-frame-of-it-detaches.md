# Amended for #2841, a page settles after a frame of it detaches, 2026-10-20

1. **A frame that detaches takes its loads with it.** A page settles once every frame of it has
   loaded, every request it made has ended, and it has been quiet for the quiet while. A frame
   that detaches (`Page.frameDetached`, other than a move to a process of its own), with every
   frame inside it, no longer counts as loading, and the requests it made in that session no
   longer count as open; a frame's session that ends drops its requests as before. Either is a
   change to the page: the quiet while starts again from it, so an act still waits for the page
   to settle after a frame goes. *D-2841-2, decided in implementation, to be confirmed.* A
   request the engine names no frame for, or one from another session, stays counted until it
   ends. *D-2841-1, decided in implementation, to be confirmed.*
