# Amended for #2967, the engine makes the page's tree before the first key, 2026-10-26

1. **The tree is made ahead of the first key.** As the person takes a browser over, and at each
   press of theirs on a page or document they have not pressed on since it loaded, purlis asks
   the engine for the root of the page's accessibility tree (`Accessibility.getRootAXNode`), on
   a thread of its own, within 30 s, and reads nothing of its answer. The take-over does not
   wait for it. The engine makes its tree the first time any of it is asked for (about 240 ms
   on the large page of #2295), so the person's first key, which reads the tree around its
   field (#2967, first amendment), finds it made. A browser handed back is asked again at the
   next take-over. *D-2967-3, decided in implementation, to be confirmed.*
2. **Not the accessibility domain.** Enabling the domain (`Accessibility.enable`), measured
   before (D-2967-2, dropped), makes the engine send the tree as events and did not bring the
   first key under 50 ms; asking for one node does not enable it.
3. **The first key is held to the bound.** The large-page real-Chrome test types its first key
   a second after the take-over, as a person does after the click that takes it over, and holds
   it to the spec's 50 ms, as every later key.
