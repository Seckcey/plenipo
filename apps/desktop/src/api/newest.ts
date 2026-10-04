/**
 * Keeps a live value's replies in order. A page reloads what Core says after every relevant
 * Ledger event, and each reload is a command Core answers on its own thread, so replies can come
 * back in any order. A reply (or a failure) takes effect only if nothing newer has been shown: no
 * reload that started after it, and no change applied from this window since it started. An old
 * reply that comes back last is dropped, so a page never goes back to what it showed before a
 * change: the Approvals queue an approval already left, a permission just changed, a role just
 * added to a connection.
 */
export class Newest {
  private started = 0;
  private shown = 0;

  /**
   * A reload is starting. Call the returned check when its reply or its failure comes back: true
   * if it may take effect, which also makes it the newest shown.
   */
  start(): () => boolean {
    const mine = ++this.started;
    return () => {
      if (mine <= this.shown) return false;
      this.shown = mine;
      return true;
    };
  }

  /** A change made from this window was applied: every reload already under way is now old. */
  applied(): void {
    this.shown = ++this.started;
  }
}
