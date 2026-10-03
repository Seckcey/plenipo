import { Button } from "@plenipo/ui";

import { BadgeList } from "./Badges";
import { messageIso, messageTime } from "./messageWords";
import { Part } from "./Part";
import { badgeWords, pointsWords, thankedWords } from "./peopleWords";
import { changeWords, pointReasonWords } from "./rewardsWords";
import { usePoints } from "./usePoints";

/**
 * **Your points** (Phase 24, ADR-169): your total and this week's, your badges with their reasons,
 * "Thanked by 12 people", and your last changes with what each was for, in plain words and your
 * own time. A change for a reason this copy of Plenipo does not know is left out.
 */
export function PointsPanel() {
  const { points, error, retry } = usePoints();
  const thanked = points ? thankedWords(points.thankedBy) : null;
  const changes = (points?.recent ?? []).flatMap((change) => {
    const words = pointReasonWords(change.reason);
    return words ? [{ ...change, words }] : [];
  });
  return (
    <Part title="Your points" hint="Points come when other people agree you helped.">
      {points === null && error === null && (
        <p className="muted" role="status">
          Looking…
        </p>
      )}
      {error !== null && (
        <>
          <p className="form-error" role="alert">
            {error}
          </p>
          <div className="settings-section__actions">
            <Button onClick={retry}>Try again</Button>
          </div>
        </>
      )}
      {points && (
        <div className="points">
          <dl className="points__totals">
            <div>
              <dt>Total</dt>
              <dd>{pointsWords(points.total)}</dd>
            </div>
            <div>
              <dt>This week</dt>
              <dd>{pointsWords(points.week)}</dd>
            </div>
          </dl>
          {thanked && <p>{thanked}</p>}
          <h3>Badges</h3>
          {points.badges.some((badge) => badgeWords(badge) !== null) ? (
            <BadgeList badges={points.badges} label="Your badges" />
          ) : (
            <p className="muted">No badges yet.</p>
          )}
          <h3>Recent changes</h3>
          {changes.length > 0 ? (
            <ul className="points__changes" aria-label="Recent changes">
              {changes.map((change, i) => (
                <li key={`${change.at}-${change.reason}-${i}`}>
                  <span className="points__amount">{changeWords(change.points)}</span>
                  <span>{change.words}</span>
                  <time className="muted" dateTime={messageIso(change.at)}>
                    {messageTime(change.at)}
                  </time>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">No changes yet.</p>
          )}
        </div>
      )}
    </Part>
  );
}
