import type { PhoneNotice, PhoneNoticeAbout } from "@plenipo/types";

/**
 * Notices on this phone (Phase 14 part 14C, ADR-144): what a notice shows, and what each of its
 * buttons does. Shared by the page and its background part (the service worker), which shows a
 * notice even when the page is closed.
 */

/** The lock screen, as this phone chose (ADR-144 §5): what it is, or only "Something needs you". */
export type LockScreen = "show" | "hide";

export const SOMETHING = "Something needs you";

/** What a notice carries back when it is tapped. */
export interface NoticeData {
  org: string;
  kind: string;
  tag: string;
  about: PhoneNoticeAbout | null;
}

/** A notice's button. */
export interface NoticeButton {
  action: string;
  title: string;
}

/**
 * What the phone shows for `notice`. The lock-screen choice comes first: hidden, a notice says
 * only "Something needs you". An approval's notice has **Approve** and **Refuse**, a lesson's
 * **Keep** and **Discard** (Android shows them; an iPhone shows none, and a tap opens the item).
 */
export function noticeFor(
  notice: PhoneNotice,
  lock: LockScreen,
): { title: string; options: NotificationOptions & { actions: NoticeButton[] } } {
  const hidden = lock === "hide";
  const about = notice.about ?? null;
  const actions: NoticeButton[] =
    about?.kind === "approval"
      ? [
          { action: "approve", title: "Approve" },
          { action: "refuse", title: "Refuse" },
        ]
      : about?.kind === "lesson"
        ? [
            { action: "keep", title: "Keep" },
            { action: "discard", title: "Discard" },
          ]
        : [];
  const data: NoticeData = { org: notice.org, kind: notice.kind, tag: notice.tag, about };
  return {
    title: hidden ? "Plenipo" : notice.title,
    options: {
      body: hidden ? SOMETHING : notice.body,
      tag: notice.tag,
      icon: "/icons/icon-192.png",
      data,
      actions,
    },
  };
}

/** Is `value` a notice this page can show (version 1)? A newer PC's notice still says something. */
export function readNotice(value: unknown): PhoneNotice | null {
  if (typeof value !== "object" || value === null) return null;
  const n = value as Partial<PhoneNotice>;
  const text = (v: unknown) => typeof v === "string";
  return n.v === 1 &&
    text(n.kind) &&
    text(n.org) &&
    text(n.title) &&
    text(n.body) &&
    text(n.tag) &&
    typeof n.at === "number"
    ? (n as PhoneNotice)
    : null;
}

/** What a tapped notice carried, checked (it came back from the phone's notice list). */
export function readNoticeData(value: unknown): NoticeData | null {
  if (typeof value !== "object" || value === null) return null;
  const d = value as Partial<NoticeData>;
  if (typeof d.org !== "string" || typeof d.kind !== "string" || typeof d.tag !== "string") {
    return null;
  }
  const about = d.about;
  const ok =
    about === null ||
    about === undefined ||
    ((about.kind === "approval" || about.kind === "lesson") && typeof about.id === "string");
  return ok ? { org: d.org, kind: d.kind, tag: d.tag, about: about ?? null } : null;
}

/** Where a tap on a notice opens the page: that approval or lesson, or the page for its kind. */
export function openTarget(data: NoticeData): string {
  if (data.about?.kind === "approval") return `approval:${data.org}:${data.about.id}`;
  if (data.about?.kind === "lesson") return `lesson:${data.org}:${data.about.id}`;
  return `${data.kind}:${data.org}`;
}

/** What tapping a notice, or one of its buttons, does. */
export type Tap =
  /** Answered from the notice, with no sign-in: only saying no (ADR-142 §5). */
  | { kind: "refuse"; org: string; approval: string }
  | { kind: "discard"; org: string; lesson: string }
  /** Plenipo opens on that item: the phone unlocks first, and asks you to sign in if needed. */
  | { kind: "open"; target: string };

export function tapOn(action: string, data: NoticeData): Tap {
  const about = data.about;
  if (action === "refuse" && about?.kind === "approval") {
    return { kind: "refuse", org: data.org, approval: about.id };
  }
  if (action === "discard" && about?.kind === "lesson") {
    return { kind: "discard", org: data.org, lesson: about.id };
  }
  // Approve, Keep, and a tap on the notice itself open the item, where one tap answers it. A
  // notice never approves anything by itself.
  return { kind: "open", target: openTarget(data) };
}

/** The page's address with an item to open (`#open=…`). */
export function openUrl(target: string): string {
  return `/#open=${encodeURIComponent(target)}`;
}

/** An item to open, from the page's address. */
export function targetFromHash(hash: string): string | null {
  const m = /^#open=(.+)$/.exec(hash.trim());
  if (!m) return null;
  try {
    const target = decodeURIComponent(m[1]!);
    return /^[a-zA-Z]+:[\w.-]{1,128}(:[\w.-]{1,128})?$/.test(target) ? target : null;
  } catch {
    return null;
  }
}

/** An item to open, read: which kind, which organization, and which one. */
export function parseTarget(target: string): { kind: string; org: string; id: string | null } {
  const [kind = "", org = "", id] = target.split(":");
  return { kind, org, id: id ?? null };
}
