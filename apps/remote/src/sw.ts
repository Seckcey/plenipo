/**
 * Plenipo's background part on the phone (Phase 14 part 14C, ADR-144): it shows a notice when
 * Plenipo's page is closed, and does what its buttons say. Built on its own as `sw.js`, from this
 * page's own files only; it loads nothing from anywhere else.
 */

import { answerFromNotice } from "./notice-answer";
import { lockScreenChoice } from "./keep";
import { SOMETHING, noticeFor, openUrl, readNotice, readNoticeData, tapOn } from "./notice";

/** The few parts of a service worker's world this uses (the DOM's types lack them). */
interface WaitUntil {
  waitUntil(work: Promise<unknown>): void;
}
interface PushMessage extends WaitUntil {
  data: { json(): unknown } | null;
}
interface NoticeTap extends WaitUntil {
  action: string;
  notification: { data: unknown; close(): void };
}
interface WindowClient {
  focus(): Promise<unknown>;
  postMessage(message: unknown): void;
}
interface Scope {
  registration: ServiceWorkerRegistration;
  clients: {
    matchAll(options: { type: "window"; includeUncontrolled: boolean }): Promise<WindowClient[]>;
    openWindow(url: string): Promise<unknown>;
    claim(): Promise<void>;
  };
  skipWaiting(): Promise<void>;
  addEventListener(type: "install" | "activate", listener: (e: WaitUntil) => void): void;
  addEventListener(type: "push", listener: (e: PushMessage) => void): void;
  addEventListener(type: "notificationclick", listener: (e: NoticeTap) => void): void;
}

declare const self: Scope;

self.addEventListener("install", (e) => e.waitUntil(self.skipWaiting()));
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));

/** A notice from the PC: shown at once (a phone must show every one it gets). */
self.addEventListener("push", (e) => {
  e.waitUntil(
    (async () => {
      const sent = (() => {
        try {
          return e.data?.json() ?? null;
        } catch {
          return null;
        }
      })();
      const notice = readNotice(sent);
      if (!notice) {
        await self.registration.showNotification("Plenipo", { body: SOMETHING, tag: "plenipo" });
        return;
      }
      // A notice sent again by mistake shows once.
      const shown = await self.registration.getNotifications({ tag: notice.tag });
      if (shown.length > 0) return;
      const { title, options } = noticeFor(notice, await lockScreenChoice());
      await self.registration.showNotification(title, options);
    })(),
  );
});

/** A notice, or one of its buttons, was tapped. */
self.addEventListener("notificationclick", (e) => {
  const data = readNoticeData(e.notification.data);
  e.notification.close();
  if (!data) return;
  const tap = tapOn(e.action, data);
  e.waitUntil(
    (async () => {
      if (tap.kind === "refuse" || tap.kind === "discard") {
        const answered = await answerFromNotice(tap);
        if (!answered.ok) {
          await self.registration.showNotification("Plenipo", {
            body: answered.message,
            tag: `${data.tag}:answer`,
            icon: "/icons/icon-192.png",
          });
        }
        return;
      }
      // Open Plenipo at that item: the open page goes there, or a new one opens.
      const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
      const open = windows[0];
      if (open) {
        open.postMessage({ type: "plenipo-open", target: tap.target });
        await open.focus();
        return;
      }
      await self.clients.openWindow(openUrl(tap.target));
    })(),
  );
});
