### Fixed

- **A browser no longer stalls for half a minute on a page whose frame keeps reloading.** A
  rotating advert or any frame that loads new documents quickly could leave purlis waiting on
  the frame it had just left, so a key you typed beside it was refused. purlis now stops
  waiting as soon as the browser says the frame has gone (#3081).
