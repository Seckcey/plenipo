export type ViewId =
  | "organization"
  | "projects"
  | "workers"
  | "approvals"
  | "runtimes"
  | "activity"
  | "settings"
  | "diagnostics";

export const VIEWS: { id: ViewId; label: string }[] = [
  { id: "organization", label: "Organization" },
  { id: "projects", label: "Projects" },
  { id: "workers", label: "Workers" },
  { id: "approvals", label: "Approvals" },
  { id: "runtimes", label: "AI tools" },
  { id: "activity", label: "Activity" },
  { id: "settings", label: "Settings" },
  { id: "diagnostics", label: "Diagnostics" },
];
