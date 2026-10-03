import { useEffect, useState } from "react";
import type { LeaderView, LeaderboardView } from "@plenipo/types";
import { Button, Segmented } from "@plenipo/ui";

import { communityLeaderboard, toCommandError } from "../api/commands";
import { BadgeList } from "./Badges";
import { Picture } from "./CommunityCard";
import { HowToEarn } from "./HowToEarn";
import { handle } from "./messageWords";
import { Part } from "./Part";
import { pointsWords } from "./peopleWords";
import { PointsPanel } from "./PointsPanel";
import { NOT_ON_BOARD, UNDER_18_BOARD, WEEK_STARTS, yourPlaceWords } from "./rewardsWords";
import { oneLine } from "./safeText";

/** The most places 8 West shows. */
const MOST_PLACES = 50;

type Which = "week" | "all";

const CHOICES: readonly { value: Which; label: string }[] = [
  { value: "week", label: "This week" },
  { value: "all", label: "All time" },
];

/**
 * One place on the leaderboard: its number, picture, names, badges, and points, and **Message**
 * (not on your own). Everything another person wrote is plain text on one line (`oneLine`), and
 * a picture is asked for only when the place says there is one.
 */
function Place({
  leader,
  you,
  onMessage,
}: {
  leader: LeaderView;
  /** This place is yours. */
  you: boolean;
  onMessage: (memberId: string, name: string) => void;
}) {
  const display = leader.displayName === null ? "" : oneLine(leader.displayName.trim());
  return (
    <li className="leaders__row">
      <span className="leaders__place">
        <span className="visually-hidden">Place </span>
        {leader.place}
      </span>
      <Picture card={leader} small />
      <div className="leaders__lines">
        {display && <p className="leaders__name">{display}</p>}
        <p className={display ? "leaders__handle" : "leaders__name"}>
          {handle(leader.name)}
          {you && <span className="leaders__you">You</span>}
        </p>
        <BadgeList badges={leader.badges} />
      </div>
      <div className="leaders__side">
        <p className="leaders__points">{pointsWords(leader.points)}</p>
        {!you && (
          <Button size="sm" onClick={() => onMessage(leader.memberId, leader.name)}>
            Message
          </Button>
        )}
      </div>
    </li>
  );
}

/**
 * **Leaderboard**: **This week** or **All time**, the top 50, and your own place. It asks 8 West
 * when it shows and when the other list is picked, and never on a timer. An answer for a list that
 * was since replaced is dropped, so the two are never mixed.
 */
function Board({
  own,
  onMessage,
}: {
  /** Your Community name, so your own place has no **Message**. */
  own: string;
  onMessage: (memberId: string, name: string) => void;
}) {
  const [which, setWhich] = useState<Which>("week");
  const [tries, setTries] = useState(0);
  const [came, setCame] = useState<{ which: Which; board: LeaderboardView } | null>(null);
  const [failed, setFailed] = useState<{ which: Which; message: string } | null>(null);
  useEffect(() => {
    let alive = true;
    communityLeaderboard(which === "all").then(
      (board) => {
        if (alive) setCame({ which, board });
      },
      (reason: unknown) => {
        if (alive) setFailed({ which, message: toCommandError(reason).message });
      },
    );
    return () => {
      alive = false;
    };
  }, [which, tries]);
  const board = came?.which === which ? came.board : null;
  const problem = failed?.which === which ? failed.message : null;
  const places = board ? board.top.slice(0, MOST_PLACES) : [];
  const choose = (next: Which) => {
    setFailed(null);
    setWhich(next);
  };
  return (
    <Part title="Leaderboard" hint="The 50 people with the most points.">
      <Segmented<Which> label="Show" value={which} options={CHOICES} onChange={choose} />
      {which === "week" && <p className="muted">{WEEK_STARTS}</p>}
      {board === null && problem === null && (
        <p className="muted" role="status">
          Looking…
        </p>
      )}
      {problem !== null && (
        <>
          <p className="form-error" role="alert">
            {problem}
          </p>
          <div className="settings-section__actions">
            <Button
              onClick={() => {
                setFailed(null);
                setTries((n) => n + 1);
              }}
            >
              Try again
            </Button>
          </div>
        </>
      )}
      {board && (
        <>
          <p className="leaders__mine">
            {board.myPlace === null ? (
              NOT_ON_BOARD
            ) : (
              <>
                <strong>{yourPlaceWords(board.myPlace)}</strong>{" "}
                <span className="muted">{pointsWords(board.myPoints)}</span>
              </>
            )}
          </p>
          {places.length === 0 ? (
            <p className="people-empty">No one is on the leaderboard yet.</p>
          ) : (
            <ol
              className="leaders"
              aria-label={`Leaderboard: ${which === "week" ? "This week" : "All time"}`}
            >
              {places.map((leader) => (
                <Place
                  key={leader.memberId}
                  leader={leader}
                  you={leader.name === own}
                  onMessage={onMessage}
                />
              ))}
            </ol>
          )}
        </>
      )}
    </Part>
  );
}

/**
 * The Leaderboard tab (Phase 24, ADR-169): **Your points**, **How to earn points**, and the board.
 * A member under 18 is not on the board and is not shown it: it is not even asked for. They are
 * told so, and see their own points.
 */
export function Leaderboard({
  adult,
  own,
  onMessage,
}: {
  /** The viewer is 18 or older. */
  adult: boolean;
  /** Your Community name. */
  own: string;
  /** Open a conversation with this person (the People tab's own way). */
  onMessage: (memberId: string, name: string) => void;
}) {
  return (
    <div className="people">
      {!adult && (
        <p className="notice-box" role="note">
          {UNDER_18_BOARD}
        </p>
      )}
      <PointsPanel />
      <HowToEarn />
      {adult && <Board own={own} onMessage={onMessage} />}
    </div>
  );
}
