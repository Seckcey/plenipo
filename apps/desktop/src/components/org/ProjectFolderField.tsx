import { useEffect, useState } from "react";
import type { OrgFolderInfo } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { chooseFolder, getOrgFolder, toCommandError } from "../../api/commands";

type Choice = "organization" | "mine";

const HINT = "Its workers' file, program, and git tools work only inside this folder.";

/**
 * Where a project's files go (ADR-205 §2.4). With an organization folder: **Make a folder in the
 * organization folder** (Plenipo keeps the project's work in its Files folder there, the default)
 * or **Use a folder I already have**, written or picked with the system's own folder chooser.
 * Without one (an organization made before 1.27), the folder box as before.
 */
export function ProjectFolderField({
  value,
  onChange,
}: {
  value: string;
  onChange: (localPath: string) => void;
}) {
  // `undefined` while asking; `null` when this organization has no organization folder.
  const [org, setOrg] = useState<OrgFolderInfo | null | undefined>(undefined);
  const [choice, setChoice] = useState<Choice>(value.trim() ? "mine" : "organization");
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    Promise.resolve()
      .then(getOrgFolder)
      .then((info) => live && setOrg(info?.path ? info : null))
      .catch(() => live && setOrg(null));
    return () => {
      live = false;
    };
  }, []);
  const choose = () => {
    setProblem(null);
    Promise.resolve()
      .then(chooseFolder)
      .then((info) => {
        if (!info?.path) return;
        if (info.problem) {
          setProblem(info.problem);
          return;
        }
        onChange(info.path);
      })
      .catch((reason: unknown) => setProblem(toCommandError(reason).message));
  };
  const folderBox = (label: string, hint: string) => (
    <label className="field">
      <span>{label}</span>
      <input
        value={value}
        maxLength={1000}
        placeholder="D:\projects\website"
        onChange={(e) => onChange(e.target.value)}
      />
      <small className="field__hint">{hint}</small>
    </label>
  );
  if (!org) {
    return folderBox(
      "Project folder (optional)",
      `${HINT} Without one, they work in Plenipo's own folder inside Documents.`,
    );
  }
  return (
    <fieldset className="choices project-folder">
      <legend>Where its files go</legend>
      <label className="choice">
        <input
          type="radio"
          name="project-folder"
          checked={choice === "organization"}
          onChange={() => {
            setChoice("organization");
            setProblem(null);
            onChange("");
          }}
        />
        <span className="choice__label">Make a folder in the organization folder</span>
      </label>
      <p className="hint">
        Plenipo makes the project a folder with a <strong>Files</strong> folder in it, inside your
        department&apos;s folder. Its workers save their work there, and you find it in Files.
      </p>
      <label className="choice">
        <input
          type="radio"
          name="project-folder"
          checked={choice === "mine"}
          onChange={() => setChoice("mine")}
        />
        <span className="choice__label">Use a folder I already have</span>
      </label>
      {choice === "mine" && (
        <div className="project-folder__line">
          {folderBox("Project folder", HINT)}
          <Button size="sm" onClick={choose}>
            Choose…
          </Button>
        </div>
      )}
      {problem && (
        <p className="form-error" role="alert">
          {problem}
        </p>
      )}
    </fieldset>
  );
}
