### Security

- **A value purlis filled no longer reaches the chat through what a page does with it.** The line
  saying a page is still loading printed each pending request's whole address, so a field that
  sends what it holds as you type put the value in the chat's answer. That line now names hosts and
  a count. Every string a browser answers a chat with, on this machine or a runner, now has the
  value taken out whole, plain, URL-encoded, form-encoded or in base64, in page text as well as in
  addresses, and the fill notice says plainly that the site receives the value and can send it to
  its servers. If you filled a value on an earlier version into a page that sends what's typed,
  treat it as seen by that chat and rotate it (#5310).
