import type { IconName } from "@plenipo/ui";

export type ViewId =
  | "organization"
  | "projects"
  | "workers"
  | "approvals"
  | "runtimes"
  | "activity"
  | "settings"
  | "diagnostics"
  | "gallery";

/** The sections on the left strip, top to bottom (Settings and Diagnostics at the bottom). */
export const VIEWS: {
  id: ViewId;
  label: string;
  icon: IconName;
  tooltip: string;
  bottom?: boolean;
}[] = [
  {
    id: "organization",
    label: "Organization",
    icon: "organization",
    tooltip: "Organization: your departments, projects, and who reports to whom",
  },
  { id: "projects", label: "Projects", icon: "projects", tooltip: "Projects and their work" },
  { id: "workers", label: "Workers", icon: "workers", tooltip: "Workers and their conversations" },
  {
    id: "approvals",
    label: "Approvals",
    icon: "approvals",
    tooltip: "Requests and lessons waiting for you",
  },
  {
    id: "runtimes",
    label: "AI tools",
    icon: "aiTools",
    tooltip: "AI tools and the programs they run",
  },
  {
    id: "activity",
    label: "Activity",
    icon: "activity",
    tooltip: "Everything that happened, in order",
  },
  { id: "settings", label: "Settings", icon: "settings", tooltip: "Settings", bottom: true },
  {
    id: "diagnostics",
    label: "Diagnostics",
    icon: "diagnostics",
    tooltip: "Diagnostics: technical details for troubleshooting",
    bottom: true,
  },
];

/** Every page's name in the top bar, including pages not on the strip. */
export const VIEW_TITLES: Record<ViewId, string> = {
  organization: "Organization",
  projects: "Projects",
  workers: "Workers",
  approvals: "Approvals",
  runtimes: "AI tools",
  activity: "Activity",
  settings: "Settings",
  diagnostics: "Diagnostics",
  gallery: "Gallery",
};

export const ALL_VIEWS = Object.keys(VIEW_TITLES) as ViewId[];
