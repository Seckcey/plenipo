// Share my profile (ADR-163 §6): one page for every name. It shows the name in this page's own
// address, as plain text, and nothing else: it never asks whether the name exists, and it sends
// nothing anywhere.

/** A Community name: 3 to 30 of a-z, 0-9, and "-", starting and ending with a letter or number. */
const NAME = /^[a-z0-9][a-z0-9-]{1,28}[a-z0-9]$/;

const [, first, name = "", ...rest] = window.location.pathname.split("/");
if (first === "c" && rest.length === 0 && NAME.test(name)) {
  for (const place of document.querySelectorAll("[data-name]")) {
    place.textContent = `@${name}`;
  }
}
