### Fixed

- **A frame its page sends back into the page's own process is usable again once its new document
  is known.** A frame from another site, shown in a process of its own, that its page navigates
  to `about:blank` was refused ("try again in a moment") until it navigated once more. It now
  follows its page's word of its new document, whether that comes before or after its old session
  ends (#5162).
