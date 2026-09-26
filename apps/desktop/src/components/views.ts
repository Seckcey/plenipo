export type ViewId = "organization" | "runtimes" | "activity" | "settings" | "diagnostics";

export const VIEWS: { id: ViewId; label: string }[] = [
  { id: "organization", label: "Organization" },
  { id: "runtimes", label: "Runtimes" },
  { id: "activity", label: "Activity" },
  { id: "settings", label: "Settings" },
  { id: "diagnostics", label: "Diagnostics" },
];
