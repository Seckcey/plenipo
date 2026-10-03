import type { WatchChange, WatchUpdate, WatchView } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import {
  applyWatchUpdate,
  countsWords,
  emptyCodeWatch,
  latestChange,
  loadWatchView,
  MAX_FILES,
  mayBeNewTeamWork,
  pinFile,
  removedWords,
  setFollowing,
  shownChange,
  splitPath,
  taskLabels,
  writingNow,
  type CodeWatch,
} from "./code";

let n = 0;
function change(over: Partial<WatchChange> = {}): WatchChange {
  n += 1;
  return {
    id: `c${n}`,
    taskId: "t1",
    sessionId: "s1",
    positionId: "p-dev",
    worker: "Senior Developer",
    objectiveTaskId: "root",
    path: "src/app.rs",
    state: "saved",
    kind: "changed",
    added: 1,
    removed: 0,
    at: 1_000 + n,
    ...over,
  };
}

const update = (c: WatchChange, writing?: string): WatchUpdate =>
  writing === undefined ? { change: c } : { change: c, writing };

function apply(state: CodeWatch, ...updates: WatchUpdate[]): CodeWatch {
  return updates.reduce(applyWatchUpdate, state);
}

const paths = (s: CodeWatch) => s.changes.map((c) => c.path);

describe("a Watch tab's list of files", () => {
  it("keeps each file's latest change, newest first, and only this agent's", () => {
    let s = emptyCodeWatch("p-dev");
    const a1 = change({ path: "a.txt", at: 10 });
    const b = change({ path: "b.txt", at: 20 });
    const a2 = change({ path: "a.txt", at: 30 });
    s = apply(s, update(a1), update(b), update(a2));
    expect(paths(s)).toEqual(["a.txt", "b.txt"]);
    expect(s.changes[0]!.id).toBe(a2.id);
    // Another agent's change is not this tab's.
    const other = apply(s, update(change({ positionId: "p-other", path: "c.txt", at: 40 })));
    expect(other).toBe(s);
    expect(latestChange(s)?.id).toBe(a2.id);
  });

  it("merges what Plenipo had when the tab opened with what it heard meanwhile, never going back in time", () => {
    // Heard while the tab read: a change still being written, then the same change saved.
    const writing = change({ path: "a.txt", state: "writing", at: 50 });
    delete writing.kind;
    let s = apply(emptyCodeWatch("p-dev"), update(writing, "fn main"));
    expect(s.writing[writing.id]).toBe("fn main");
    const view: WatchView = {
      positionId: "p-dev",
      objectiveTaskId: "root",
      // Newest first, as the hub lists them; "a.txt" as it was before (older).
      changes: [
        change({ path: "b.txt", at: 40 }),
        { ...writing, state: "writing", at: 45 },
        change({ path: "c.txt", at: 30 }),
      ],
      fromTheRecord: false,
      teamTaskIds: [],
    };
    s = loadWatchView(s, view);
    expect(paths(s)).toEqual(["a.txt", "b.txt", "c.txt"]);
    // The older copy of "a.txt" did not replace the one heard later, nor its text.
    expect(s.changes[0]!.at).toBe(50);
    expect(s.writing[writing.id]).toBe("fn main");
    // A late update (older than what the tab has) changes nothing.
    const late = apply(s, update({ ...writing, at: 20 }, "fn"));
    expect(late).toBe(s);
    // Saved: the text so far is dropped (the saved file is read from Plenipo instead).
    s = apply(s, update({ ...writing, state: "saved", kind: "created", at: 60 }));
    expect(s.changes[0]!.state).toBe("saved");
    expect(s.writing).toEqual({});
  });

  it("keeps the text so far only while a change is being written or waits", () => {
    const c = change({ path: "a.txt", state: "writing", at: 10 });
    let s = apply(emptyCodeWatch("p-dev"), update(c, "one"), update({ ...c, at: 11 }, "one\ntwo"));
    expect(s.writing[c.id]).toBe("one\ntwo");
    expect(writingNow(s)).toBe(true);
    s = apply(s, update({ ...c, state: "waiting", at: 12 }, "one\ntwo"));
    expect(s.writing[c.id]).toBe("one\ntwo");
    expect(writingNow(s)).toBe(false);
    s = apply(s, update({ ...c, state: "refused", reason: "Not approved", at: 13 }));
    expect(s.writing).toEqual({});
    // A newer change to the same file drops the older one's text.
    const d = change({ path: "b.txt", state: "writing", at: 20 });
    s = apply(s, update(d, "secret-free text"));
    const e = change({ path: "b.txt", state: "writing", at: 30 });
    s = apply(s, update(e, "x"));
    expect(Object.keys(s.writing)).toEqual([e.id]);
  });

  it("starts a fresh list for a new objective, and ignores an older objective's late news", () => {
    let s = apply(
      emptyCodeWatch("p-dev"),
      update(change({ path: "a.txt", at: 10 })),
      update(change({ path: "b.txt", at: 20 })),
    );
    s = pinFile(s, "a.txt");
    s = apply(s, update(change({ path: "c.txt", objectiveTaskId: "root2", at: 30 })));
    expect(paths(s)).toEqual(["c.txt"]);
    expect(s.objectiveTaskId).toBe("root2");
    expect(s.following).toBe(true);
    // The first objective's change that turned "not saved" when its task ended, heard late.
    const late = apply(s, update(change({ path: "d.txt", state: "notSaved", at: 15 })));
    expect(late).toBe(s);
  });

  it("does not swap objectives while a change in this one is being written or waits", () => {
    // An on-call agent with work in two objectives at once.
    const a = change({ path: "a.txt", state: "writing", at: 10 });
    let s = apply(
      emptyCodeWatch("p-dev"),
      update(a, "one"),
      update(change({ path: "b.txt", at: 20 })),
    );
    s = pinFile(s, "b.txt");
    const elsewhere = update(change({ path: "b.txt", objectiveTaskId: "root2", at: 30 }));
    expect(apply(s, elsewhere)).toBe(s);
    // Once nothing here is open, the other objective's next change starts a fresh list; the
    // pinned file stays pinned, as it is in the new list.
    s = apply(s, update({ ...a, state: "saved", at: 40 }));
    s = apply(s, update(change({ path: "b.txt", objectiveTaskId: "root2", at: 50 })));
    expect(s.objectiveTaskId).toBe("root2");
    expect(paths(s)).toEqual(["b.txt"]);
    expect(s.following).toBe(false);
    expect(s.pinned).toBe("b.txt");
    // A change waiting for approval holds the list too.
    s = apply(
      s,
      update(change({ path: "c.txt", objectiveTaskId: "root2", state: "waiting", at: 60 }), "x"),
    );
    expect(apply(s, update(change({ path: "d.txt", at: 70 })))).toBe(s);
    // Following along stays on when the list is swapped.
    s = setFollowing(
      apply(s, update(change({ path: "c.txt", objectiveTaskId: "root2", at: 80 }))),
      true,
    );
    s = apply(s, update(change({ path: "d.txt", at: 90 })));
    expect(s.objectiveTaskId).toBe("root");
    expect(s.following).toBe(true);
  });

  it("keeps what it heard over what Plenipo had, when both are as new", () => {
    // Written and saved in the same millisecond; Plenipo's list was read in between.
    const c = change({ path: "a.txt", state: "writing", at: 50 });
    let s = apply(emptyCodeWatch("p-dev"), update(c, "fn main"));
    s = apply(s, update({ ...c, state: "saved", kind: "created" }));
    expect(s.changes[0]!.state).toBe("saved");
    const view: WatchView = {
      positionId: "p-dev",
      objectiveTaskId: "root",
      changes: [{ ...c, state: "writing" }],
      fromTheRecord: false,
      teamTaskIds: [],
    };
    expect(loadWatchView(s, view)).toBe(s);
    // Nor does another objective's change read as new as the newest start a fresh list.
    const other: WatchView = {
      positionId: "p-dev",
      objectiveTaskId: "root2",
      changes: [change({ path: "b.txt", objectiveTaskId: "root2", at: 50 })],
      fromTheRecord: false,
      teamTaskIds: [],
    };
    expect(loadWatchView(s, other).objectiveTaskId).toBe("root");
  });

  it("shows its team's changes, and asks again about a hand-off it doesn't know (Phase 25, item 1.8)", () => {
    // A Supervisor's Watch: it wrote nothing itself, its developer did.
    let s = loadWatchView(emptyCodeWatch("p-lead"), {
      positionId: "p-lead",
      objectiveTaskId: "root",
      changes: [change({ path: "a.rs", taskId: "t-dev", positionId: "p-dev" })],
      fromTheRecord: false,
      teamTaskIds: ["t-dev", "t-lead"],
    });
    expect(paths(s)).toEqual(["a.rs"]);
    // Its team's next change is heard live.
    s = apply(s, update(change({ path: "b.rs", taskId: "t-dev", positionId: "p-dev" })));
    expect(paths(s)).toEqual(["b.rs", "a.rs"]);
    // Work handed on since: not shown yet, and the tab reads Plenipo's list again.
    const fresh = update(change({ path: "c.rs", taskId: "t-qa", positionId: "p-qa" }));
    expect(applyWatchUpdate(s, fresh)).toBe(s);
    expect(mayBeNewTeamWork(s, fresh)).toBe(true);
    // Another objective's work is not this tab's to ask about.
    expect(
      mayBeNewTeamWork(s, update(change({ objectiveTaskId: "root2", positionId: "p-qa" }))),
    ).toBe(false);
  });

  it("says why Watch is empty when Plenipo knows (Phase 25, item 1.8)", () => {
    const s = loadWatchView(emptyCodeWatch("p-dev"), {
      positionId: "p-dev",
      objectiveTaskId: "root",
      changes: [],
      fromTheRecord: false,
      teamTaskIds: ["t1"],
      quiet: "Senior Developer got no tools: this work belongs to no project.",
    });
    expect(s.quiet).toBe("Senior Developer got no tools: this work belongs to no project.");
    // Its objective is known before any change.
    expect(s.objectiveTaskId).toBe("root");
  });

  it("keeps at most a few hundred files", () => {
    let s = emptyCodeWatch("p-dev");
    for (let i = 0; i < MAX_FILES + 25; i++) {
      s = applyWatchUpdate(
        s,
        update(change({ path: `f${i}.txt`, state: "writing", at: 10 + i }), `text ${i}`),
      );
    }
    expect(s.changes).toHaveLength(MAX_FILES);
    expect(s.changes[0]!.path).toBe(`f${MAX_FILES + 24}.txt`);
    expect(paths(s)).not.toContain("f0.txt");
    expect(Object.keys(s.writing)).toHaveLength(MAX_FILES);
  });

  it("says when its changes are from before Plenipo started again", () => {
    const s = loadWatchView(emptyCodeWatch("p-dev"), {
      positionId: "p-dev",
      objectiveTaskId: "root",
      changes: [change({ summary: "The lines are shown only while Plenipo runs" })],
      fromTheRecord: true,
      teamTaskIds: [],
    });
    expect(s.fromTheRecord).toBe(true);
    // Another agent's view is not this tab's.
    const other = loadWatchView(emptyCodeWatch("p-dev"), {
      positionId: "p-other",
      changes: [change({ positionId: "p-other" })],
      fromTheRecord: true,
      teamTaskIds: [],
    });
    expect(other.changes).toEqual([]);
  });
});

describe("Follow along and Pin this file", () => {
  it("follows the newest change by default; a pinned file stays in view", () => {
    let s = apply(
      emptyCodeWatch("p-dev"),
      update(change({ path: "a.txt", at: 10 })),
      update(change({ path: "b.txt", at: 20 })),
    );
    expect(s.following).toBe(true);
    expect(shownChange(s)?.path).toBe("b.txt");
    s = pinFile(s, "a.txt");
    s = apply(s, update(change({ path: "c.txt", at: 30 })));
    expect(shownChange(s)?.path).toBe("a.txt");
    // A newer change to the pinned file shows.
    const a2 = change({ path: "a.txt", at: 40 });
    s = apply(s, update(a2), update(change({ path: "d.txt", at: 50 })));
    expect(shownChange(s)?.id).toBe(a2.id);
    // Follow along again: the newest shows.
    s = setFollowing(s, true);
    expect(s.pinned).toBeNull();
    expect(shownChange(s)?.path).toBe("d.txt");
    // Pin this file, from the toggle: the file shown now stays.
    s = setFollowing(s, false);
    expect(s.pinned).toBe("d.txt");
    s = apply(s, update(change({ path: "e.txt", at: 60 })));
    expect(shownChange(s)?.path).toBe("d.txt");
  });

  it("shows the newest when there is nothing to pin", () => {
    expect(shownChange(emptyCodeWatch("p-dev"))).toBeNull();
    const s = setFollowing(emptyCodeWatch("p-dev"), false);
    expect(s.following).toBe(false);
    const next = apply(s, update(change({ path: "a.txt" })));
    expect(shownChange(next)?.path).toBe("a.txt");
  });
});

describe("words", () => {
  it("says counts, removed lines, paths, and tasks plainly", () => {
    expect(countsWords({ added: 1, removed: 2 })).toBe("1 line added, 2 removed");
    expect(countsWords({ added: 3, removed: 1 })).toBe("3 lines added, 1 removed");
    expect(removedWords(1)).toBe("1 line removed here");
    expect(removedWords(3)).toBe("3 lines removed here");
    expect(splitPath("src/terminal/code.ts")).toEqual(["code.ts", "src/terminal/"]);
    expect(splitPath("README.md")).toEqual(["README.md", ""]);
    const one = [change({ taskId: "t1" }), change({ taskId: "t1" })];
    expect(taskLabels(one).size).toBe(0);
    const two = [change({ taskId: "t2", at: 5_000 }), change({ taskId: "t1", at: 10 })];
    expect(taskLabels(two).get("t1")).toBe("Task 1");
    expect(taskLabels(two).get("t2")).toBe("Task 2");
  });
});
