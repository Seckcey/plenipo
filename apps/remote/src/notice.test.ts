import type { PhoneNotice } from "@plenipo/types";
import { describe, expect, it } from "vitest";

import {
  SOMETHING,
  noticeFor,
  openUrl,
  parseTarget,
  readNotice,
  readNoticeData,
  tapOn,
  targetFromHash,
} from "./notice";

const approval: PhoneNotice = {
  v: 1,
  kind: "approvals",
  org: "first",
  title: "Senior Developer is waiting for your OK",
  body: "Git push origin",
  about: { kind: "approval", id: "a1" },
  tag: "approval:first:a1",
  at: 1,
};

describe("what a notice shows", () => {
  it("shows what it is, with Approve and Refuse for an approval", () => {
    const { title, options } = noticeFor(approval, "show");
    expect(title).toBe("Senior Developer is waiting for your OK");
    expect(options.body).toBe("Git push origin");
    expect(options.tag).toBe("approval:first:a1");
    expect(options.actions.map((a) => a.title)).toEqual(["Approve", "Refuse"]);
    expect(options.data).toEqual({
      org: "first",
      kind: "approvals",
      tag: "approval:first:a1",
      about: { kind: "approval", id: "a1" },
    });
  });

  it("shows only “Something needs you” when the phone chose that", () => {
    const { title, options } = noticeFor(approval, "hide");
    expect(title).toBe("Plenipo");
    expect(options.body).toBe(SOMETHING);
    expect(JSON.stringify({ title, body: options.body })).not.toContain("git push");
    // The buttons still answer the same approval.
    expect(options.actions.map((a) => a.title)).toEqual(["Approve", "Refuse"]);
  });

  it("gives a lesson Keep and Discard, and anything else no buttons", () => {
    const lesson = noticeFor(
      { ...approval, kind: "lessons", about: { kind: "lesson", id: "l1" } },
      "show",
    );
    expect(lesson.options.actions.map((a) => a.title)).toEqual(["Keep", "Discard"]);
    const finished = noticeFor(
      {
        v: 1,
        kind: "finished",
        org: "first",
        title: "An objective is finished",
        body: "Relaunch the website",
        tag: "finished:first:x",
        at: 1,
      },
      "show",
    );
    expect(finished.options.actions).toEqual([]);
  });

  it("reads only a notice it knows, and says something for any other", () => {
    expect(readNotice(approval)).toEqual(approval);
    expect(readNotice({ ...approval, v: 2 })).toBeNull();
    expect(readNotice({ ...approval, title: 3 })).toBeNull();
    expect(readNotice("hello")).toBeNull();
    expect(readNotice(null)).toBeNull();
  });
});

describe("what tapping a notice does", () => {
  const data = readNoticeData(noticeFor(approval, "show").options.data)!;

  it("Refuse answers from the notice; Approve, and a tap, open the approval", () => {
    expect(tapOn("refuse", data)).toEqual({ kind: "refuse", org: "first", approval: "a1" });
    // A notice never approves anything by itself.
    expect(tapOn("approve", data)).toEqual({ kind: "open", target: "approval:first:a1" });
    expect(tapOn("", data)).toEqual({ kind: "open", target: "approval:first:a1" });
  });

  it("Discard answers a lesson from the notice; Keep opens it", () => {
    const lesson = { ...data, about: { kind: "lesson" as const, id: "l1" } };
    expect(tapOn("discard", lesson)).toEqual({ kind: "discard", org: "first", lesson: "l1" });
    expect(tapOn("keep", lesson)).toEqual({ kind: "open", target: "lesson:first:l1" });
    // A button that does not fit what the notice is about only opens it.
    expect(tapOn("refuse", lesson)).toEqual({ kind: "open", target: "lesson:first:l1" });
  });

  it("opens a notice about nothing in particular at its kind", () => {
    expect(tapOn("", { ...data, kind: "finished", about: null })).toEqual({
      kind: "open",
      target: "finished:first",
    });
  });

  it("keeps the item in the address, and reads back only a sound one", () => {
    const url = openUrl("approval:first:a1");
    expect(url).toBe("/#open=approval%3Afirst%3Aa1");
    expect(targetFromHash(url.slice(1))).toBe("approval:first:a1");
    expect(parseTarget("approval:first:a1")).toEqual({ kind: "approval", org: "first", id: "a1" });
    expect(parseTarget("finished:first")).toEqual({ kind: "finished", org: "first", id: null });
    expect(targetFromHash("#open=javascript:alert(1)")).toBeNull();
    expect(targetFromHash("#open=approval:first:../../x")).toBeNull();
    expect(targetFromHash("#pair=ABC")).toBeNull();
  });

  it("refuses notice data that is not what a notice carries", () => {
    expect(readNoticeData(null)).toBeNull();
    expect(readNoticeData({ org: "first", kind: "approvals" })).toBeNull();
    expect(
      readNoticeData({
        org: "first",
        kind: "approvals",
        tag: "t",
        about: { kind: "run", id: "x" },
      }),
    ).toBeNull();
  });
});
