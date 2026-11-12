### Fixed

- **An artifact's bar shows the offers its page made last.** A page that changed what it offers
  twice in quick succession could be left showing the earlier set, so an offer such as Mark done
  never appeared. purlis now draws each set in the order the page made them, and draws none
  from a page that has since loaded again (#6261).
