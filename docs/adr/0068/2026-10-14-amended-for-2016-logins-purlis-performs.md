# Amended for #2016, logins purlis performs, 2026-10-14

A persona's declared browser login is performed by purlis as the browser's third driver, and its
values go from the vault to the session host and the browser, never to the chat. That is the
operator's ruling of 2026-10-14 (spec #2008's grill): purlis takes the lease, goes to the
declared address, fills only while the page's origin is the declared one, submits, waits for the
field to go, clears it on failure, and hands back; a browser holding a login reaches only its
declared origins. The amendments above are left as accepted, and this note adds to them. Items 1
to 5 are **decided in implementation, to be confirmed** (D-2016-1 to D-2016-5).

1. **Session protocol 1.18, a minor.** `log_in` carries a declared login to a browser: the login
   page's address and the values from the vault, answered `browsed` with the page the login left
   once the browser is handed back, or refused `browser_refused` in words that carry no value.
   The values never show in a command's `Debug`, and are wiped from memory when dropped. It is
   `local-ui`'s alone, with a row of its own in FD-27's table (`Power::LogIn`): no relayed link
   carries it, so a login's values reach no other host. It keeps its order in the browser's lane
   with the browser's acts. Recorded in the protocol's fixture. *D-2016-1, to be confirmed.*
2. **purlis holds the lease while it logs in.** The engine takes the lease from the agent under a
   new epoch, as a take-over does: an act of the agent's in flight is cut, and every act is
   refused in words until the login answers. The person may still take the browser over, which
   cuts the login and leaves the browser theirs. Otherwise the engine hands the browser back to
   the agent under a new epoch, every reference dealt before it void, and the host takes that
   epoch as the agent's lease. No login runs while the person drives. *D-2016-2, to be
   confirmed.*
3. **What purlis fills is kept out of every answer, as what the person types is.** The values
   themselves are kept out of every answer about every page of the browser, a page opened later
   included, for as long as the browser lives; each field purlis fills is blanked before its
   value goes in. *D-2016-3, to be confirmed.*
4. **It fills only on the declared origin, in a page of its own.** purlis opens the login's page
   itself, so no other page opened it, and fills only while the engine records that page's own
   document on the declared origin; only that document is searched for the form. Right before
   each value goes in and right after, the page must still show the same document with the field
   focused in it; where it does not, purlis ends the whole browser before the agent has it again.
   It submits by Enter. All it can tell is that the page, on the declared origin, shows no
   password field in two reads in a row, and it says so, never that the person is signed in (a
   page may ask for a second step); within the login's limit (thirty seconds for the form to
   show, and again for the field to go, a setting like every browser limit), or it fails and what
   it filled is cleared. A login the person cuts by taking the browser over has what it filled
   cleared as they hand it back. *D-2016-4, to be confirmed.*
5. **A browser holding a login reaches its login's site alone, from the first fill.** Just
   before the first value goes in, the browser's reach proxy is bound to the login's host and
   port, ends every connection it carried until then, and from then on carries nothing else for
   as long as the browser runs, whatever lists anything else; nobody is asked about another site,
   since no Allow would carry it. The site is the login page's one host and port, which is
   narrower than the ruling's "declared origins". A browser that holds a login at one site is not
   logged in at another: the chat opens another browser. A login page's site is carried while its
   login runs, and stays carried only once the browser is bound to it. *D-2016-5, to be
   confirmed.*
