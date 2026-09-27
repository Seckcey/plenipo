import { useRef, useState } from "react";
import type { AppInfo } from "@plenipo/types";
import { Icon, Tabs, useStoredState } from "@plenipo/ui";

import { ModelSettings } from "../components/models/ModelSettings";
import { PermissionSettings } from "../components/permissions/PermissionSettings";
import { ServerSettings } from "../components/servers/ServerSettings";
import { SwitchSettings } from "../components/SwitchSettings";
import { TitlesSetting } from "../components/TitlesSetting";
import type { Go } from "../components/views";
import { LearningSwitch } from "../learning/Lessons";
import { useLearning } from "../learning/useLearning";
import {
  AboutPlenipo,
  AiToolsSettings,
  DiagnosticsSummary,
  LocalPathsSettings,
  OrganizationSettings,
} from "../settings/InfoSettings";
import { NotificationSettings } from "../settings/NotificationSettings";
import {
  SETTINGS_SECTIONS,
  SETTINGS_SECTION_KEY,
  isSettingsSection,
  type SettingsSection,
} from "../settings/sections";
import { TerminalSettings } from "../settings/TerminalSettings";

/**
 * Settings in one place (Phase 12): a list of sections on the left, one section at a time, and
 * the last one comes back. Another page can open a section (Home: "Fix it in Settings →
 * Servers"). Settings → Servers is the same as before, in its own section.
 */
export function SettingsView({
  go,
  info,
  section: asked = null,
}: {
  go: Go;
  info: AppInfo | null;
  /** A section another page asked for. */
  section?: string | null;
}) {
  const learning = useLearning();
  const [stored, setStored] = useStoredState<SettingsSection>(
    SETTINGS_SECTION_KEY,
    "aiTools",
    isSettingsSection,
  );
  // A section asked for opens once; after that the owner moves freely.
  const [arrived, setArrived] = useState<string | null>(null);
  let current = stored;
  if (asked && asked !== arrived && isSettingsSection(asked)) {
    setArrived(asked);
    setStored(asked);
    current = asked;
  }
  const meta = SETTINGS_SECTIONS.find((s) => s.id === current) ?? SETTINGS_SECTIONS[0]!;
  const panel = useRef<HTMLDivElement>(null);
  /** Another section opens at its top (the page scrolls as one). */
  const choose = (next: SettingsSection) => {
    setStored(next);
    panel.current?.closest("main")?.scrollTo?.({ top: 0 });
  };

  return (
    <section className="view settings-view" aria-labelledby="settings-title">
      <h1 id="settings-title">Settings</h1>
      <div className="settings-layout">
        <Tabs<SettingsSection>
          label="Settings sections"
          orientation="vertical"
          idPrefix="settings"
          className="settings-layout__list"
          value={current}
          onChange={choose}
          tabs={SETTINGS_SECTIONS.map((s) => ({
            value: s.id,
            label: (
              <>
                <Icon name={s.icon} size={16} />
                <span>{s.label}</span>
              </>
            ),
          }))}
        />
        <div
          ref={panel}
          className="settings-layout__panel"
          role="tabpanel"
          id={`settings-panel-${current}`}
          aria-labelledby={`settings-tab-${current}`}
        >
          <h2 id={`settings-${current}`} className="settings-layout__title">
            {meta.label}
          </h2>
          <p className="view__lead">{meta.lead}</p>
          {current === "aiTools" && <AiToolsSettings go={go} />}
          {current === "aiModels" && <ModelSettings />}
          {current === "permissions" && <PermissionSettings />}
          {current === "organization" && <OrganizationSettings go={go} />}
          {current === "servers" && <ServerSettings />}
          {current === "switches" && (
            <SwitchSettings learning={<LearningSwitch learning={learning} />} />
          )}
          {current === "notifications" && <NotificationSettings />}
          {current === "terminal" && <TerminalSettings />}
          {current === "personalization" && (
            <>
              <TitlesSetting />
              <p className="muted">
                Light or dark: use the button at the top right. Plenipo remembers your choice.
              </p>
            </>
          )}
          {current === "localPaths" && <LocalPathsSettings />}
          {current === "diagnostics" && <DiagnosticsSummary go={go} info={info} />}
          {current === "about" && <AboutPlenipo info={info} />}
        </div>
      </div>
    </section>
  );
}
