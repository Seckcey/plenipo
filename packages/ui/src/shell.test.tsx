import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import {
  AppShell,
  Banner,
  BannerSlot,
  IconRail,
  NotificationBell,
  ScopeSelector,
  ThemeToggle,
  TopBar,
} from "./shell";
import { useTheme } from "./theme";

type Section = "home" | "projects" | "approvals" | "settings";

function Rail() {
  const [current, setCurrent] = useState<Section>("home");
  return (
    <IconRail<Section>
      current={current}
      onSelect={setCurrent}
      items={[
        { id: "home", label: "Home", icon: "home" },
        {
          id: "projects",
          label: "Projects",
          icon: "projects",
          tooltip: "Your projects and their work",
        },
        {
          id: "approvals",
          label: "Approvals",
          icon: "approvals",
          badge: { count: 2, label: "waiting for you" },
        },
        { id: "settings", label: "Settings", icon: "settings", bottom: true },
      ]}
    />
  );
}

describe("IconRail", () => {
  it("shows each section's name under its icon, marks the current one, and has tooltips", async () => {
    const user = userEvent.setup();
    render(<Rail />);
    const nav = screen.getByRole("navigation", { name: "Main" });
    const home = within(nav).getByRole("button", { name: "Home" });
    expect(home).toHaveAttribute("aria-current", "page");
    expect(within(home).getByText("Home")).toBeVisible();
    expect(within(nav).getByRole("button", { name: "Projects" })).toHaveAttribute(
      "title",
      "Your projects and their work",
    );
    expect(within(nav).getByLabelText("2 waiting for you")).toHaveTextContent("2");
    await user.click(within(nav).getByRole("button", { name: /Approvals/ }));
    expect(within(nav).getByRole("button", { name: /Approvals/ })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(home).not.toHaveAttribute("aria-current");
  });

  it("is reachable with Tab, in order, and works with Enter", async () => {
    const user = userEvent.setup();
    render(<Rail />);
    await user.tab();
    expect(screen.getByRole("button", { name: "Home" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Projects" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Projects" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "Settings" })).toHaveFocus();
  });
});

function Themed() {
  const [theme, setTheme] = useTheme();
  return <ThemeToggle theme={theme} onChange={setTheme} />;
}

describe("ThemeToggle", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
  });

  it("switches light and dark, and remembers the choice", async () => {
    const user = userEvent.setup();
    const { unmount } = render(<Themed />);
    await user.click(screen.getByRole("button", { name: "Switch to the light theme" }));
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("plenipo.theme")).toBe("light");
    expect(screen.getByRole("button", { name: "Switch to the dark theme" })).toBeInTheDocument();
    unmount();
    render(<Themed />);
    expect(screen.getByRole("button", { name: "Switch to the dark theme" })).toBeInTheDocument();
  });
});

describe("TopBar", () => {
  it("has the scope, the title, the bell, and the theme switch", async () => {
    const user = userEvent.setup();
    const open = vi.fn();
    const scope = vi.fn();
    render(
      <AppShell
        rail={<Rail />}
        topBar={
          <TopBar
            start={
              <ScopeSelector
                value="org"
                onChange={scope}
                options={[
                  { id: "org", label: "All of 8 West", kind: "organization" },
                  { id: "d1", label: "Development", kind: "department" },
                  { id: "p1", label: "Website", kind: "project" },
                ]}
              />
            }
            title="Projects"
            end={<NotificationBell count={3} label="waiting for your approval" onOpen={open} />}
          />
        }
      >
        <main>Page</main>
      </AppShell>,
    );
    expect(screen.getByRole("banner")).toHaveTextContent("Projects");
    expect(screen.queryByRole("heading")).toBeNull();
    const select = screen.getByRole("combobox", { name: "Showing" });
    expect(within(select).getByRole("group", { name: "Departments" })).toBeInTheDocument();
    await user.selectOptions(select, "p1");
    expect(scope).toHaveBeenCalledWith("p1");
    await user.click(
      screen.getByRole("button", { name: "Notifications: 3 waiting for your approval" }),
    );
    expect(open).toHaveBeenCalled();
  });

  it("says when nothing is waiting", () => {
    render(
      <NotificationBell count={0} label="waiting for your approval" onOpen={() => undefined} />,
    );
    expect(
      screen.getByRole("button", { name: "Notifications: Nothing waiting for your approval" }),
    ).toBeInTheDocument();
  });
});

describe("Banner", () => {
  it("has its call to action and Dismiss", async () => {
    const user = userEvent.setup();
    const review = vi.fn();
    const dismiss = vi.fn();
    render(
      <BannerSlot>
        <Banner
          tone="pending"
          title="Senior Developer is waiting for your approval"
          action={<button onClick={review}>Review</button>}
          onDismiss={dismiss}
        >
          git push origin main
        </Banner>
      </BannerSlot>,
    );
    const region = screen.getByRole("region", { name: "Notices" });
    const banner = within(region).getByRole("status");
    expect(banner).toHaveTextContent("Senior Developer is waiting for your approval");
    await user.click(within(banner).getByRole("button", { name: "Review" }));
    await user.click(within(banner).getByRole("button", { name: "Dismiss" }));
    expect(review).toHaveBeenCalled();
    expect(dismiss).toHaveBeenCalled();
  });

  it("uses alert for errors", () => {
    render(<Banner tone="error" title="The Ledger failed its integrity check" />);
    expect(screen.getByRole("alert")).toHaveTextContent("integrity check");
  });
});
