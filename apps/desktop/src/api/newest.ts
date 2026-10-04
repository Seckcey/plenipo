/**
 * Keeps a live value's replies in order. A page reloads what Core says after every relevant
 * Ledger event, and each reload is a command Core answers on its own thread, so replies can come
 * back in any order. A reply takes effect only if nothing newer has been shown: no reload that
 * started after it and answered, and no change applied from this window since it started. An old
 * reply that comes back last is dropped, so a page never goes back to what it showed before a
 * change: the Approvals queue an approval already left, a permission just changed, a role just
 * added to a connection.
 *
 * A failure is shown only if nothing newer has been shown, but it never counts as shown itself:
 * an older reply that comes back after a newer failure still shows (and clears the failure).
 */
export class Newest {
  private started = 0;
  private shown = 0;

  /** A reload is starting. Use the returned checks when its reply or its failure comes back. */
  start(): Look {
    const mine = ++this.started;
    return {
      take: () => {
        if (mine <= this.shown) return false;
        this.shown = mine;
        return true;
      },
      fresh: () => mine > this.shown,
    };
  }

  /** A change made from this window was applied: every reload already under way is now old. */
  applied(): void {
    this.shown = ++this.started;
  }
}

/** One reload's checks, for when it comes back. */
export interface Look {
  /** Its reply came back: true if it may show, which makes it the newest shown. */
  take(): boolean;
  /** It failed: true if nothing newer has been shown, so the failure may show. */
  fresh(): boolean;
}
