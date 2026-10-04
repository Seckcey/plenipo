/**
 * Core after a change, in a test. A page reads Core again a moment after every change it makes
 * (and after the Ledger events the change records), so a stand-in for Core must answer that read
 * the way Core would. Once `command` has been called, it answers `answer`, and `loader` answers
 * the same from then on. Before the change, `loader` answers whatever the test set up.
 *
 * `answer` is the value, or a function of the command's arguments that builds it.
 */
export function afterChange<A extends unknown[], T>(
  command: { mockImplementation(fn: (...args: A) => Promise<T>): unknown },
  answer: T | ((...args: A) => T),
  loader: { mockResolvedValue(value: T): unknown },
): void {
  command.mockImplementation((...args: A) => {
    const value = typeof answer === "function" ? (answer as (...args: A) => T)(...args) : answer;
    loader.mockResolvedValue(value);
    return Promise.resolve(value);
  });
}
