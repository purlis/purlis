# Amended for #2311, one text per field, 2026-10-17

Spec #2008 rules that what the person typed never reaches the model (the operator's ruling of
2026-10-14). The amendment for #2014, item 4, keeps what a field holds out of every later answer,
read after each key; this note says which of those reads are kept. Nothing else changes, and the
session protocol does not.

1. **A field keeps its latest value.** Each text kept out is kept with the element it was read
   from. A read of a field the person typed into, from the field itself, drops that field's
   earlier values that the new value contains, while they are shorter than 8 characters; an
   earlier value of 8 characters or more stays kept out, so a page that shows a long value cut
   short still shows nothing of it. A value typed key by key is kept out as the value, not as each
   short part of it on the way. An earlier value the new one does not contain (one the person
   deleted back from) stays kept out. A read of a page's tree drops nothing. Each run of
   characters as the person typed it, what a key such as a paste put into the field, and what the
   field held right after that key, are kept out as they were entered and never dropped, nor is a
   text read from another element. This is
   the person's ruling of 2026-10-17. *D-2311-1, ruled.*
2. **A text is kept on the site of the document its page shows.** The site a text is kept out on,
   whose pages' screenshots paint over what cannot be read (the amendment for #2303, item 2), is
   the one of the document the page has loaded when the text is kept, not of an address the page
   is still loading; until the page's document is known, it is the one of its address. Whether a
   page's screenshots paint over what cannot be read is decided by the same site. A page whose
   document is on no site counts as its opener's, or as kept when it has none. *D-2311-2, to be
   confirmed.*
