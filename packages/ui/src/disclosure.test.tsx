import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { Button } from "./controls";
import { Disclosure } from "./disclosure";

describe("Disclosure (Phase 25, item 2.2)", () => {
  beforeEach(() => localStorage.clear());

  it("starts closed, showing its name, light, line, and buttons; the rest shows when opened", async () => {
    const user = userEvent.setup();
    const signIn = vi.fn();
    render(
      <Disclosure
        title="Claude Code"
        summary="Subscription connected · Claude Max"
        status={{ status: "ok", label: "Subscription connected" }}
        actions={
          <Button size="sm" onClick={signIn}>
            Reconnect
          </Button>
        }
      >
        <p>Installed 2.1.999</p>
      </Disclosure>,
    );
    const card = screen.getByRole("region", { name: "Claude Code" });
    const toggle = within(card).getByRole("button", { name: "Claude Code" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(card).toHaveTextContent("Subscription connected · Claude Max");
    expect(screen.queryByText("Installed 2.1.999")).toBeNull();
    // A header button works closed, and never opens or closes the card.
    await user.click(within(card).getByRole("button", { name: "Reconnect" }));
    expect(signIn).toHaveBeenCalledTimes(1);
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Installed 2.1.999")).toBeVisible();
    await user.click(toggle);
    expect(screen.queryByText("Installed 2.1.999")).toBeNull();
  });

  it("shows header buttons meant only for a closed card only while it is closed", async () => {
    const user = userEvent.setup();
    render(
      <Disclosure
        title="Codex"
        actions={(open) => (open ? null : <Button size="sm">Sign in</Button>)}
      >
        <Button size="sm">Sign in</Button>
      </Disclosure>,
    );
    expect(screen.getAllByRole("button", { name: "Sign in" })).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Codex" }));
    expect(screen.getAllByRole("button", { name: "Sign in" })).toHaveLength(1);
  });

  it("remembers that it was left open", async () => {
    const user = userEvent.setup();
    const card = (
      <Disclosure title="Slack" rememberAs="connections:slack">
        <p>Parts</p>
      </Disclosure>
    );
    const first = render(card);
    await user.click(screen.getByRole("button", { name: "Slack" }));
    first.unmount();
    render(card);
    expect(screen.getByRole("button", { name: "Slack" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Parts")).toBeVisible();
  });

  it("opens by itself when it starts needing you, and can be closed again", async () => {
    const user = userEvent.setup();
    function Host() {
      const [needs, setNeeds] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setNeeds(true)}>
            Break it
          </button>
          <Disclosure title="Codex" openWhen={needs}>
            <p>Sign in again</p>
          </Disclosure>
        </>
      );
    }
    render(<Host />);
    const toggle = screen.getByRole("button", { name: "Codex" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await user.click(screen.getByRole("button", { name: "Break it" }));
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    // Still needing you, but you closed it: it stays closed.
    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  it("starts open when it needs you, whatever was remembered", () => {
    localStorage.setItem("disclosure:ai:codex", "false");
    render(
      <Disclosure title="Codex" rememberAs="ai:codex" openWhen>
        <p>Sign in again</p>
      </Disclosure>,
    );
    expect(screen.getByRole("button", { name: "Codex" })).toHaveAttribute("aria-expanded", "true");
  });
});
