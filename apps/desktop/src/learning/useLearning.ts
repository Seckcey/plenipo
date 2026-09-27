import type { LearningSnapshot } from "@plenipo/types";

import { getLearning } from "../api/commands";
import { useLive } from "../guard/usePermissions";

/** Ledger events after which learning may look different. */
export function affectsLearning(eventType: string): boolean {
  return (
    eventType.startsWith("lesson.") ||
    eventType.startsWith("learning.") ||
    eventType === "org.settings_changed" ||
    eventType.startsWith("org.role_")
  );
}

/** Learning (ADR-022), kept live: the settings and the lessons waiting and kept. */
export function useLearning() {
  const { value, error, reload, apply } = useLive<LearningSnapshot>(getLearning, affectsLearning);
  return { snapshot: value, error, reload, apply };
}

export type Learning = ReturnType<typeof useLearning>;
