# Amendment, 2026-10-01: the rename's compat window is six months (V37c)

§1's project-format migration said: *"For a compat window of at least one release, charter
still reads the old names"*. **Ruled by V37c:** *"Old forms are read for six months from the
first release that writes the new form. #762's plane→project read-compat window uses the same
six months. This amends ADR 0072's "at least one release"."*

The window is therefore **six months from the first release that writes the new names**. During
it purlis still reads the old names, writes only the new ones and says so once when it reads an
old one, as §1 says. It is the same window `docs/plane-format.md` gives every format change
(*Compatibility across purlis versions*, FR-24). The rest of ADR 0072 stands.
