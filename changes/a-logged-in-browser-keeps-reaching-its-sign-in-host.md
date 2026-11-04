### Changed

- **A browser login that signs in on another host keeps reaching that host, and the chat acts on
  its pages only while it signs in** (#5152). The site's own redirects, hidden frames and token
  refreshes to its sign-in host work after the login. Outside a sign-in, a chat's call on a page
  of the sign-in host is refused with the reason, a frame of it inside the site's page is left
  out of what the chat reads and painted over in screenshots, and a page that asks to sign in
  again tells the chat to log in again. The login's answer lists the hosts the browser reaches,
  and what the chat may do on each.
