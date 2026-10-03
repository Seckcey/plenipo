import { EARN_INVITES, EARN_LIMITS, EARN_ROWS } from "./rewardsWords";

/**
 * **How to earn points** (Phase 24, ADR-169 §1, ADR-172 §3): the eight ways, each with its points,
 * then what earns nothing and the limits, and which invitations count. A section you open.
 */
export function HowToEarn() {
  return (
    <details className="earn">
      <summary>How to earn points</summary>
      <div className="earn__body">
        <table className="earn__table" aria-label="Ways to earn points">
          <thead>
            <tr>
              <th scope="col">What happens</th>
              <th scope="col">Points</th>
            </tr>
          </thead>
          <tbody>
            {EARN_ROWS.map((row) => (
              <tr key={row.what}>
                <td>{row.what}</td>
                <td className="earn__points">{row.points}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p>{EARN_LIMITS}</p>
        <p>{EARN_INVITES}</p>
      </div>
    </details>
  );
}
