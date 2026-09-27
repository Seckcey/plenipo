import type { ControlSession } from "@plenipo/types";

/** What a session looks like in words. */
export function sessionWords(s: ControlSession): { title: string; detail: string } {
  const what = s.kind === "browser" ? "Plenipo's browser" : "your mouse and keyboard";
  const detail = [s.detail, s.lastAction].filter(Boolean).join(" · ");
  switch (s.state) {
    case "active":
      return { title: `${s.worker} is using ${what}`, detail };
    case "takenOver":
      return {
        title:
          s.kind === "browser"
            ? `You have control of the browser. ${s.worker} stopped.`
            : `You took back the mouse and keyboard. ${s.worker} stopped.`,
        detail,
      };
    case "stopped":
      return { title: `Stopped by you: ${s.worker}`, detail };
  }
}
