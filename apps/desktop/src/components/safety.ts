/** Settings → Safety's choices and warnings (ADR-201), in plain words. */
import type { Safety } from "@plenipo/types";

import { systemWords } from "../system/words";

/** The three choices in the order they are shown, with what each one means (ADR-201). */
export const SAFETY_CHOICES: readonly { value: Safety; label: string; says: string }[] = [
  {
    value: "light",
    label: "Light",
    says: "Agents save files and run programs and scripts without asking you. This is how Plenipo starts.",
  },
  {
    value: "careful",
    label: "Careful",
    says: "Agents save files without asking. A program that is not on your approved list, and every PowerShell script, asks you first.",
  },
  {
    value: "strict",
    label: "Strict",
    says: "Agents only read. They save nothing and run nothing.",
  },
];

/** The warning for the choice that is on now, in plain words. */
export function safetyWarning(choice: Safety): string {
  switch (choice) {
    case "light":
      return `Read this once. On Light, an agent can create, change, and delete files in its own folder, and run programs and scripts, without asking you. A program or script can do anything you can do on ${systemWords().thisComputer}, and Plenipo cannot see everything inside one. Plenipo still stops the programs on your Never run list and any file outside the agent's folder, and it asks you before an agent sends work out, like pushing code to GitHub. Choose Careful if you want to be asked first.`;
    case "careful":
      return "On Careful, agents still create, change, and delete files in their own folder without asking you. Choose Strict if you want them to only read.";
    case "strict":
      return "On Strict, agents only read. Work that needs to save a file or run a program stops and says so.";
  }
}
