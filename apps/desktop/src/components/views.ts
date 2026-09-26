export type ViewId =
  "organization" | "workers" | "runtimes" | "activity" | "settings" | "diagnostics";

export const VIEWS: { id: ViewId; label: string }[] = [
  { id: "organization", label: "Organization" },
  { id: "workers", label: "Workers" },
  { id: "runtimes", label: "Runtimes" },
  { id: "activity", label: "Activity" },
  { id: "settings", label: "Settings" },
  { id: "diagnostics", label: "Diagnostics" },
];
