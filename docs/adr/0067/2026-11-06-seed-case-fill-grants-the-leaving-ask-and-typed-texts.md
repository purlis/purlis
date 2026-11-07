# Amended (2026-11-06, spec #5100; the person's rulings of that day, and the steward's decisions beside them): a seed in any case, what a fill grant holds, what answers a leaving ask, and long typed texts

Builds on the amendments of 2026-11-04 and 2026-11-05 on secret fill, its grants, the seed look
and the hold of a filled page.

*Ruled by the person, and built:*
- **A seed in any case mix.** The seed look counts a run of base32's alphabet whatever its mix of
  upper and lower case, as it counted one in a single case. A name joined to the value by `_` or
  `-` (`secret_GEZD…`) no longer keeps it from being read as a seed. A password of 16 or more
  characters drawn from base32's alphabet alone (the letters, and the digits 2 to 7) needs the
  person's no-seed mark. One with a 0, 1, 8 or 9, or any other mark, fills as before.
- **A grant fills no field sent by GET.** Under Allow for this chat or Always, a fill into a field
  whose form submits by GET is refused with no notice: the page's address would carry the value
  into the site's logs and the browser's history. Asked about, Allow once fills it.
- **Only an answer to the leaving ask counts.** A held page's redirect to a site other than the
  one its sign-in began from goes by itself only where the person answered the ask about that
  leaving. An earlier Allow of the site, given when the browser asked about it for anything else,
  no longer counts. Each browser asks at least once.

*Decided by the steward, and built:*
- **A payment field asks each time.** A field the page marks with an `autocomplete` token that
  begins `cc-` raises the notice under either grant, with Allow once and Keep blocked alone,
  whatever its key is marked. The notice says it as a card or payment value.
- **A narrowed chat fills under its narrowed grants.** A chat a handoff or a dispatch narrowed to
  another persona's grants reads that persona's fill lines and login grants, and none where it
  holds none. Its notice's Always writes in that persona's tools. The vaults it reads stay the
  persona it was started as.
- **A download holding a long typed text stays out.** `browser_save_download` refuses a file
  holding a text of 8 or more characters that the browser keeps out of a chat's reads, one the
  person typed included, as it refuses one holding a filled value.
- **A site whose name holds a long typed text is not named.** A site the browser asks about is
  left out of what the chat is told where its name holds such a text, one typed into a single
  page included. A shorter one still hides no site: it is often a word of a site's name.

*Decided in implementation:*
- **The field's facts ride with no new protocol minor.** The engine's field read says whether
  the field's form sends it by GET (its own `method`, or any submit button's `formmethod`, that
  isn't `post`) and whether the field is marked for payment. Both are optional in the protocol.
  A window takes a read that says neither as not known, and asks the person about the fill. A
  host that doesn't read them is never sent a fill under a grant.
- **The engine judges again as it writes.** A fill under a grant says so to the engine. The
  function that writes the value refuses a field sent by GET, or one marked for payment, in the
  same step as the write, so a page that changes its form between the read and the fill is
  caught. The attributes are read, never the form's properties, which a control named `method`
  stands in for.
- **An Allow from before the ask is set aside until the window has seen the ask.** The window
  drops a site from the sites the person allowed when asked whenever its browser asks about that
  site again. The judge counts a site only where its browser asked to leave for it, and not while
  the Allow it holds is one it held when it asked.
- **8 characters is `KEPT_WHOLE_FROM`**, the length from which a field's earlier values stay kept
  (the ruling of 2026-10-17). The download check reads the file once and says a filled value
  before a typed text.
- **One reading names the narrowed persona** (`Chats::secret_grants_of`): the persona held by
  the chat's `held` grants where it has them, else the persona it was started as. Every read of a
  login's Allow from its notice takes it, as do the fill's grants and both asks. A chat with no
  sandbox reads its declarations at the call, so a narrowed one's login must be one the persona it
  was narrowed to declares and this machine allowed.
