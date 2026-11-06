### Security

- **A sign-in's download is its own page's.** While `browser_login` signs in, a download from the
  site it signs in on, or from no site purlis can tell, is brought into the chat's folder only
  where the page it signs in on, or a window related to it, began it; one begun on another tab is
  kept out as that tab is kept off the site.
- **A frame of the site a login signs in on takes no input inside another site's frame.** A click
  aimed at a frame of that site, drawn inside a frame of another site that runs in a process of
  its own, is refused as one on the frame itself is.
