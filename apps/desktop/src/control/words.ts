import type { ControlSession } from "@plenipo/types";

/** What a session looks like in words. */
export function sessionWords(s: ControlSession): { title: string; detail: string } {
  if (s.kind === "server") {
    const where = s.detail ?? "a server";
    const detail = s.lastAction ?? "";
    switch (s.state) {
      case "active":
        return { title: `${s.worker} is connected to ${where}`, detail };
      case "takenOver":
        return { title: `You disconnected ${s.worker} from ${where}. It stopped.`, detail };
      case "stopped":
        return { title: `Stopped by you: ${s.worker} (servers)`, detail };
    }
  }
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

/** The owner's button for an active session: servers are disconnected, the rest taken over. */
export function takeOverLabel(s: ControlSession): string {
  return s.kind === "server" ? "Disconnect" : "Take over";
}
