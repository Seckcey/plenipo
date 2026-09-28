import type {
  AgentRuntimeInfo,
  AiToolsPage,
  AiToolState,
  KnownModel,
  ToolInfo,
} from "@plenipo/types";
import { Button, StatusPill } from "@plenipo/ui";

import { checkAiTool } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { when } from "../../pages/words";
import { EFFORT_LABEL } from "../../routing/format";
import { Refusal } from "../models/shared";

/** "Opus (opus) · Effort: Low, Medium, High". */
function ModelWords({ model }: { model: KnownModel }) {
  return (
    <>
      <strong>{model.label}</strong>
      {model.label !== model.name && (
        <>
          {" "}
          <code>{model.name}</code>
        </>
      )}
      <span className="muted">
        {" · "}
        {model.effortLevels.length > 0
          ? `Effort: ${model.effortLevels.map((e) => EFFORT_LABEL[e]).join(", ")}`
          : "No effort setting"}
      </span>
    </>
  );
}

/**
 * A card's Models (ADR-060 §5): the models Plenipo checked, and the ones the tool reported that it
 * has not ("new — not checked yet", which you can still choose); a checked model the tool no
 * longer lists is "not offered by this version". Check again asks the tool now.
 */
export function ModelsTab({
  info,
  tool,
  route,
  onApply,
}: {
  info: AgentRuntimeInfo;
  tool: AiToolState | undefined;
  route: ToolInfo | undefined;
  onApply: (page: AiToolsPage) => void;
}) {
  const { pending, error, run } = useRun<AiToolsPage>(onApply);
  const label = info.label;
  const known = info.capabilities.knownModels;
  const unlisted = new Set(route?.unlistedModels ?? []);
  const fresh = route
    ? route.newModels
    : (info.reportedModels?.models ?? []).filter((m) => !known.some((k) => k.name === m.name));
  const reported = info.reportedModels;

  return (
    <div className="ai-tool__block">
      {known.length + fresh.length === 0 ? (
        <p className="muted">No models listed yet: {label} uses its own default.</p>
      ) : (
        <ul className="ai-tool__list" aria-label={`${label}'s models`}>
          {known.map((m) => (
            <li key={m.name}>
              <ModelWords model={m} />
              {unlisted.has(m.name) && (
                <>
                  {" "}
                  <StatusPill status="warn" label="not offered by this version" />
                </>
              )}
            </li>
          ))}
          {fresh.map((m) => (
            <li key={m.name}>
              <ModelWords model={m} /> <StatusPill status="pending" label="new — not checked yet" />
            </li>
          ))}
        </ul>
      )}
      {fresh.length > 0 && (
        <p className="muted">
          You can choose a new model in the model menus. Plenipo has not checked it with this
          version of {label} yet.
        </p>
      )}
      {tool && !tool.hasModelList ? (
        <p>{label} has no list of its own. Its models come with Plenipo&apos;s updates.</p>
      ) : reported ? (
        <p className="muted">
          Plenipo last asked {label} at {when(reported.checkedAt)}.
          {!reported.complete && ` ${label} lists only the models on this computer.`}
        </p>
      ) : (
        tool && <p className="muted">Plenipo hasn&apos;t asked {label} for its models yet.</p>
      )}
      {tool?.modelsCheckLeavesATrace && (
        <p className="muted">
          Asking {label} for its models leaves an empty conversation in {label}&apos;s own history,
          so Plenipo asks only after an update and when you press Check again.
        </p>
      )}
      <div className="ai-tool__buttons">
        <Button
          size="sm"
          icon="refresh"
          disabled={pending || tool?.checking === true}
          aria-label={`Check ${label} again`}
          onClick={() => void run(() => checkAiTool(info.id))}
        >
          {pending || tool?.checking ? "Checking…" : "Check again"}
        </Button>
      </div>
      <Refusal error={error} />
    </div>
  );
}
