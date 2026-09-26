export type ViewId =
  "organization" | "workers" | "approvals" | "runtimes" | "activity" | "settings" | "diagnostics";

export const VIEWS: { id: ViewId; label: string }[] = [
  { id: "organization", label: "Organization" },
  { id: "workers", label: "Workers" },
  { id: "approvals", label: "Approvals" },
  { id: "runtimes", label: "AI tools" },
  { id: "activity", label: "Activity" },
  { id: "settings", label: "Settings" },
  { id: "diagnostics", label: "Diagnostics" },
];
