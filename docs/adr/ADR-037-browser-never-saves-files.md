# ADR-037: Plenipo's browser does not save files

- **Status:** Accepted (by the owner, 2026-09-27)
- **Date:** 2026-09-27
- **Phase:** 10 (follow-up)
- **Amends:** [ADR-020 (Plenipo's browser and computer use)](ADR-020-browser-and-computer-use.md),
  section 2 (Plenipo's browser) and its known limits

## Context

Plenipo's browser is a copy of Microsoft Edge or Google Chrome that Plenipo starts with its own
profile (ADR-020). Like any browser, it could save files: a link that says `download`, or a page
whose answer says "attachment", puts a file in the Downloads folder. Until now that happened in
Plenipo's browser as in any other. A page a worker opened could put a file on the owner's
computer with no approval, no record in the Activity trail, and nothing said to the worker. A
worker's click could land on such a link without meaning to, and a page can start a download by
itself, with no click at all.

That breaks two promises: nothing from a website changes the owner's computer without the owner
knowing, and every significant action is recorded. A saved file is also a way for a website to
hand a worker something Plenipo never looked at.

## Decision

1. **Plenipo's browser never saves files.** Right after the browser starts and shows it is up,
   and before any tab exists, Plenipo tells it to refuse every download (the DevTools protocol's
   `Browser.setDownloadBehavior` with `deny`, told to the browser as a whole, so every tab is
   covered, the owner's sign-in tab too). There is no approval for saving a file, no folder to
   save to, and no setting that turns saving on.
2. **A browser that cannot be told so is not used.** If the browser does not take the setting,
   the start fails with a plain-words error ("Microsoft Edge could not be set to never save files
   (…), so Plenipo did not use it"), shown in Settings like any other browser problem, and the
   browser is stopped. Plenipo never falls back to a browser that may save files.
3. **The worker is told.** The browser reports each download it refused
   (`Browser.downloadWillBegin`, which Edge and Chrome still send under `deny`; the file is
   canceled before a byte of it is written). The report names only the frame that started the
   download, so each tab keeps the frames of its page, and the note goes to the worker whose tab
   it was, with its next result: "The page tried to save a file (report.pdf). Plenipo's browser
   does not save files." Nobody is told of a download from a tab no worker has (the owner's own).
4. **Nothing else changes.** A worker that needs what a file holds reads the page it is on, or
   asks its lead. There is no tool that saves a file from a website, on purpose.

## Consequences

- Websites that hand out files (a report, an export, an invoice as a PDF) cannot be used that way
  by workers. The click does nothing visible on the page, and the worker's next result says why.
  The owner can save such a file in their own browser.
- The owner's own tab in Plenipo's browser (opened from Settings to sign in to a website) cannot
  save files either, and neither can "Save page as" there. The owner's own browser is for that.
- The refusal leaves no file and changes nothing on the computer, so there is nothing for the
  Activity trail; the worker's result carries the note, and the Ledger keeps the result.

**Known limits:**

- The note depends on the browser reporting refused downloads. Edge and Chrome do today. Should a
  future version stop, the click would just do nothing visible and the worker would not be told,
  but no file would be saved either way: refusing comes first, and it does not depend on the
  report.
- A file the browser shows inside a tab (a PDF in its viewer, an image) is a page, not a download:
  it opens, and the worker reads what the page helper can read of it.

## Alternatives considered

- **Asking the owner for each download**, as for sending. Not chosen: an approval belongs to a
  worker's tool call, but a download can begin with no call running (the page starts it by
  itself), and an approved file would sit in a folder that no tool of a worker's reads. If workers
  ever need files from websites, that is a new tool with its own record and approval, not a
  browser setting.
- **Saving into a folder of Plenipo's own** (`allowAndName` with a path in Plenipo's data
  folder). Not chosen: the file still lands on the computer unasked and unread, only elsewhere.
- **Watching downloads and deleting them.** Not chosen: the file exists for a moment, a fast
  download wins the race, and a browser that never writes it is simpler.
