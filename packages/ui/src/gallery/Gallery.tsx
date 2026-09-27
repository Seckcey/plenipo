/**
 * The Gallery: every component in the library, in every state, in either theme or both side
 * by side (Phase 12A). The desktop app shows it from Diagnostics, with real data from the
 * Ledger in the first section.
 */

import { useMemo, useState, type ReactNode } from "react";

import { PlenipoLogo, PlenipoMark, Pip } from "../brand";
import { PIP_POSES } from "../brand-data";
import {
  CardGrid,
  EntityCard,
  EntityCardSkeleton,
  type CollectionView,
  type EntityCardProps,
} from "../cards";
import {
  Button,
  Checkbox,
  IconButton,
  SearchField,
  Segmented,
  Select,
  Switch,
  Tabs,
  TextField,
} from "../controls";
import { DetailSplitView, PropertyList, TimelineScrubber, type TimelineValue } from "../detail";
import { LogView } from "../log";
import { MenuButton, ResizeHandle } from "../menu";
import { TERMINAL_FONT } from "../terminal-theme";
import { EMPTY_FACETS, useFacets, type FacetConfig } from "../facet-logic";
import { FacetPanel } from "../facets";
import { ICON_NAMES } from "../icon-data";
import { Icon } from "../icons";
import { Banner } from "../shell";
import { EmptyState, ErrorState, LoadingState } from "../states";
import {
  ActivityStrip,
  ActivityStripPlaceholder,
  CountBadge,
  HealthBar,
  Sparkline,
  StatusDot,
  StatusPill,
} from "../status";
import { STATUSES } from "../status-types";
import { CellLink, DataTable } from "../table";
import { statusColumn, type Column } from "../table-columns";
import {
  COLOR_ROLES,
  FONT_SIZE,
  RADIUS,
  SPACE,
  colorVar,
  type ColorToken,
  type ThemeName,
} from "../tokens";
import { TopologyMap } from "../topology";
import { DAY_MS } from "../activity";
import {
  sampleActivity,
  sampleCards,
  sampleMap,
  sampleTimeline,
  sampleWorkers,
  STATUS_WORDS,
  type SampleWorker,
} from "./fixtures";

/** Real data for the first section, from the app (the Ledger). */
export interface GalleryLive {
  state: "loading" | "error" | "ready";
  error?: string;
  /** Each card with a unique id (a department and a project can share a name). */
  cards: (EntityCardProps & { id: string })[];
}

function Section({
  id,
  title,
  children,
  lead,
}: {
  id: string;
  title: string;
  lead?: string;
  children: ReactNode;
}) {
  return (
    <section className="gallery__section" aria-labelledby={id} data-gallery-section={id}>
      <h2 id={id}>{title}</h2>
      {lead && <p className="gallery__lead">{lead}</p>}
      {children}
    </section>
  );
}

/** One sample. `name` is for the look test (`data-gallery`); `caption` is what people read. */
function Variant({
  name,
  caption,
  children,
  wide = false,
}: {
  name: string;
  caption: string;
  children: ReactNode;
  wide?: boolean;
}) {
  return (
    <figure
      className={wide ? "gallery__variant gallery__variant--wide" : "gallery__variant"}
      data-gallery={name}
    >
      <figcaption>{caption}</figcaption>
      <div className="gallery__sample">{children}</div>
    </figure>
  );
}

const TABLE_ROWS = 5000;
const GRID_CARDS = 150;

function workerColumns(onOpenProject: (name: string) => void): Column<SampleWorker>[] {
  return [
    { id: "name", header: "Name", cell: (w) => w.name, sortValue: (w) => w.name, width: "220px" },
    statusColumn((w) => ({ status: w.status, label: STATUS_WORDS[w.status] })),
    { id: "tool", header: "AI tool", cell: (w) => w.tool, sortValue: (w) => w.tool },
    {
      id: "project",
      header: "Project",
      cell: (w) => <CellLink onClick={() => onOpenProject(w.project)}>{w.project}</CellLink>,
      sortValue: (w) => w.project,
    },
    {
      id: "department",
      header: "Department",
      cell: (w) => w.department,
      sortValue: (w) => w.department,
    },
    {
      id: "tasks",
      header: "Tasks",
      numeric: true,
      cell: (w) => w.tasks,
      sortValue: (w) => w.tasks,
    },
    {
      id: "minutes",
      header: "Minutes",
      numeric: true,
      cell: (w) => w.minutes.toFixed(1),
      sortValue: (w) => w.minutes,
    },
    {
      id: "cost",
      header: "Usage ($)",
      numeric: true,
      cell: (w) => w.cost.toFixed(2),
      sortValue: (w) => w.cost,
      hidden: true,
    },
    {
      id: "seen",
      header: "Last seen",
      numeric: true,
      cell: (w) =>
        w.hoursAgo < 24 ? `${w.hoursAgo} h ago` : `${Math.round(w.hoursAgo / 24)} d ago`,
      sortValue: (w) => w.hoursAgo,
    },
  ];
}

const facetConfig: FacetConfig<SampleWorker> = {
  search: (w) => `${w.name} ${w.project} ${w.tool}`,
  groups: [
    { id: "status", label: "Status", value: (w) => w.status, labels: STATUS_WORDS },
    { id: "tool", label: "AI tool", value: (w) => w.tool },
    { id: "department", label: "Department", value: (w) => w.department },
  ],
  ranges: [
    {
      id: "seen",
      label: "Last seen",
      min: 0,
      max: 2160,
      step: 24,
      value: (w) => w.hoursAgo,
      format: (h) => (h === 0 ? "Now" : h < 48 ? `${h} h` : `${Math.round(h / 24)} d`),
      marks: [
        { value: 0, label: "Now" },
        { value: 720, label: "1M" },
        { value: 2160, label: "3M" },
      ],
    },
  ],
};

function FilteredTable({ workers, prefix }: { workers: SampleWorker[]; prefix: string }) {
  const facets = useFacets(workers, facetConfig);
  const [collapsed, setCollapsed] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [opened, setOpened] = useState<string | null>(null);
  const columns = useMemo(() => workerColumns(setOpened), []);
  return (
    <div className="gallery__filtered" data-gallery="filtered-table">
      <FacetPanel
        state={facets.state}
        onChange={facets.setState}
        groups={facets.groups}
        ranges={facets.ranges}
        collapsed={collapsed}
        onCollapsedChange={setCollapsed}
        total={workers.length}
        shown={facets.filtered.length}
        searchPlaceholder="Search workers"
        label="Filter workers"
      />
      <div className="gallery__filtered-main">
        {opened && (
          <p className="gallery__note" role="status">
            Opened project {opened}
          </p>
        )}
        <DataTable
          label="Workers"
          rows={facets.filtered}
          columns={columns}
          getRowId={(w) => w.id}
          selectable
          selected={selected}
          onSelectedChange={setSelected}
          defaultPageSize="all"
          storageKey={`plenipo.gallery.${prefix}.table`}
          height={420}
        />
      </div>
    </div>
  );
}

function workerCard(w: SampleWorker, now: number): EntityCardProps {
  return {
    title: w.name,
    status: w.status,
    statusLabel: STATUS_WORDS[w.status],
    subtype: `${w.project} · ${w.department}`,
    activity: sampleActivity(Number(w.id.slice(1)), now, 0.5),
    owner: { icon: "aiTools", label: w.tool },
    resources: [
      { icon: "file", label: "Read and change files" },
      { icon: "terminal", label: "Run programs" },
    ],
  };
}

function Grid({ workers, now }: { workers: SampleWorker[]; now: number }) {
  const [view, setView] = useState<CollectionView>("cards");
  const [collapsed, setCollapsed] = useState(false);
  const facets = useFacets(workers, facetConfig);
  const cards = useMemo(
    () => new Map(workers.map((w) => [w.id, workerCard(w, now)])),
    [workers, now],
  );
  return (
    <div className="gallery__filtered" data-gallery="filtered-grid">
      <FacetPanel
        state={facets.state}
        onChange={facets.setState}
        groups={facets.groups}
        collapsed={collapsed}
        onCollapsedChange={setCollapsed}
        total={workers.length}
        shown={facets.filtered.length}
        searchPlaceholder="Search cards"
        label="Filter worker cards"
      />
      <div className="gallery__filtered-main">
        <CardGrid
          label="Worker cards"
          items={facets.filtered}
          getKey={(w) => w.id}
          view={view}
          onViewChange={setView}
          height={400}
          renderCard={(w) => (
            <EntityCard {...(cards.get(w.id) ?? workerCard(w, now))} onOpen={() => undefined} />
          )}
          renderRow={(w) => (
            <>
              <StatusDot status={w.status} label={STATUS_WORDS[w.status]} />
              <strong>{w.name}</strong>
              <span className="gallery__muted">{w.project}</span>
              <span className="gallery__muted">{w.tool}</span>
            </>
          )}
        />
      </div>
    </div>
  );
}

function SplitSample({ now, prefix }: { now: number; prefix: string }) {
  const [value, setValue] = useState<TimelineValue>("live");
  const [live, setLive] = useState(true);
  const [map, setMap] = useState(true);
  const [selected, setSelected] = useState<string | null>("web");
  const { nodes, links } = useMemo(() => sampleMap(), []);
  const events = useMemo(() => sampleTimeline(now), [now]);
  const rows = useMemo(() => sampleWorkers(12, 3), []);
  const columns = useMemo(() => workerColumns(() => undefined).slice(0, 6), []);
  return (
    <div data-gallery="split">
      <DetailSplitView
        label="Website Supervisor"
        properties={
          <PropertyList
            title={<StatusDot status="pending" label="Website Supervisor · Waiting for you" />}
            items={[
              { label: "AI tool", value: "Codex" },
              { label: "Model", value: "Automatic" },
              { label: "Project", value: "Website" },
              {
                label: "Live updates",
                value: <Switch label="Live updates" checked={live} onChange={setLive} />,
              },
              {
                label: "Show the map",
                value: <Switch label="Show the map" checked={map} onChange={setMap} />,
              },
            ]}
          />
        }
        timeline={
          <TimelineScrubber
            from={now - DAY_MS}
            to={now}
            events={events}
            value={value}
            onChange={setValue}
            label="Website Supervisor's timeline"
          />
        }
        map={
          map ? (
            <TopologyMap
              label="Delegation tree"
              nodes={nodes}
              links={links}
              selectedId={selected}
              onSelect={setSelected}
            />
          ) : (
            <EmptyState compact title="The map is hidden" />
          )
        }
        table={
          <DataTable
            label={`Team at ${value === "live" ? "now" : "the chosen time"}`}
            rows={rows}
            columns={columns}
            getRowId={(w) => w.id}
            defaultPageSize={25}
            storageKey={`plenipo.gallery.${prefix}.split`}
            height={260}
          />
        }
      />
    </div>
  );
}

/** What the terminal looks like: its text and the 16 colors, from the tokens (as CSS). */
function TerminalSample() {
  const names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"] as const;
  return (
    <pre className="gallery__terminal" style={{ fontFamily: TERMINAL_FONT.family }}>
      <span>deploy@shop:~$ systemctl status nginx{"\n"}</span>
      {["", "bright-"].map((bright) => (
        <span key={bright}>
          {names.map((n) => (
            <span key={n} style={{ color: colorVar(`terminal-${bright}${n}` as ColorToken) }}>
              {`${bright}${n} `}
            </span>
          ))}
          {"\n"}
        </span>
      ))}
    </pre>
  );
}

function Swatch({ token }: { token: ColorToken }) {
  return (
    <div className="gallery__swatch" data-gallery={`color-${token}`}>
      <span className="gallery__chip" style={{ background: colorVar(token) }} />
      <span>
        <code>{token}</code>
        <span className="gallery__muted">{COLOR_ROLES[token]}</span>
      </span>
    </div>
  );
}

function GalleryBody({ now, live, prefix }: { now: number; live?: GalleryLive; prefix: string }) {
  const [on, setOn] = useState(true);
  const [checked, setChecked] = useState(true);
  const [search, setSearch] = useState("");
  const [text, setText] = useState("Website");
  const [choice, setChoice] = useState<"a" | "b">("a");
  const [tab, setTab] = useState<"one" | "two" | "three">("one");
  const [terminals, setTerminals] = useState(["pc", "shop", "watch"]);
  const [terminal, setTerminal] = useState("shop");
  const [section, setSection] = useState<"tools" | "servers" | "about">("servers");
  const [panelSize, setPanelSize] = useState(160);
  const [view, setView] = useState<CollectionView>("cards");
  const [dismissed, setDismissed] = useState(false);
  const cards = useMemo(() => sampleCards(now), [now]);
  const workers = useMemo(() => sampleWorkers(TABLE_ROWS), []);
  const gridWorkers = useMemo(() => workers.slice(0, GRID_CARDS), [workers]);
  const { nodes, links } = useMemo(() => sampleMap(), []);
  const activity = useMemo(() => sampleActivity(5, now, 1.2), [now]);
  const spark = useMemo(() => activity.buckets.map((b) => b.events), [activity]);
  const smallColumns = useMemo(() => workerColumns(() => undefined).slice(0, 4), []);

  return (
    <div className="gallery__body">
      {live && (
        <Section
          id={`${prefix}-live`}
          title="From your organization"
          lead="Real departments and projects, with activity from the Ledger."
        >
          {live.state === "loading" ? (
            <div className="gallery__cards">
              <EntityCardSkeleton />
              <EntityCardSkeleton />
            </div>
          ) : live.state === "error" ? (
            <ErrorState title="Couldn't read the organization" message={live.error} />
          ) : live.cards.length === 0 ? (
            <EmptyState title="No departments or projects yet" icon="organization">
              Add them on the Organization page.
            </EmptyState>
          ) : (
            <div className="gallery__cards" data-gallery="live-cards">
              {live.cards.map(({ id, ...card }) => (
                // Real activity ends at the moment it was read, so "Now" follows the clock.
                <EntityCard key={id} {...card} />
              ))}
            </div>
          )}
        </Section>
      )}

      <Section
        id={`${prefix}-status`}
        title="Status"
        lead="A mark and a word, never color alone. Each status has its own mark shape."
      >
        <div className="gallery__row">
          <Variant name="status-dot" caption="Status marks">
            {STATUSES.map((s) => (
              <StatusDot key={s} status={s} label={STATUS_WORDS[s]} />
            ))}
          </Variant>
          <Variant name="status-pill" caption="Status labels">
            {STATUSES.map((s) => (
              <StatusPill key={s} status={s} label={STATUS_WORDS[s]} />
            ))}
          </Variant>
          <Variant name="count-badge" caption="Count badges">
            <CountBadge count={3} label="waiting for you" />
            <CountBadge count={12} label="working" tone="ok" />
            <CountBadge count={2} label="blocked" tone="warn" />
            <CountBadge count={1} label="failed" tone="error" />
            <CountBadge count={0} label="waiting" showZero tone="offline" />
          </Variant>
        </div>
        <div className="gallery__row">
          <Variant name="health-bar" caption="Health bars">
            <HealthBar label="Tasks done" value={7} max={10} valueText="7 of 10" />
            <HealthBar label="Usage limit" value={82} max={100} status="warn" />
            <HealthBar label="Checks passing" value={2} max={9} valueText="2 of 9" status="error" />
          </Variant>
          <Variant name="sparkline" caption="Sparklines">
            <Sparkline values={spark} label="Events per 15 minutes" />
            <Sparkline values={spark.slice(40)} label="Events" status="ok" />
            <Sparkline values={[]} label="Events" />
          </Variant>
        </div>
        <div className="gallery__row">
          <Variant name="activity-strip" caption="24-hour activity" wide>
            <ActivityStrip series={activity} now={now} />
          </Variant>
          <Variant
            name="activity-strip-states"
            caption="Activity: loading, couldn't load, nothing yet"
            wide
          >
            <ActivityStripPlaceholder state="empty" />
            <ActivityStripPlaceholder state="loading" />
            <ActivityStripPlaceholder state="error" />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-cards`}
        title="Cards"
        lead="A department, project, or worker: status, 24-hour activity, who fills it, and its permissions."
      >
        <div className="gallery__cards">
          {cards.map((c) => (
            <div key={c.title} data-gallery={`card-${c.status}`}>
              <EntityCard
                {...c}
                now={now}
                onOpen={() => undefined}
                selected={c.title === "Website"}
              />
            </div>
          ))}
          <div data-gallery="card-loading-activity">
            <EntityCard
              title="Operations"
              status="ok"
              statusLabel="Working"
              subtype="Department"
              activity="loading"
            />
          </div>
          <div data-gallery="card-error-activity">
            <EntityCard
              title="Marketing"
              status="warn"
              statusLabel="Blocked"
              subtype="Department"
              activity="error"
            />
          </div>
          <div data-gallery="card-skeleton">
            <EntityCardSkeleton />
          </div>
        </div>
      </Section>

      <Section
        id={`${prefix}-grid`}
        title="Card grid"
        lead={`${GRID_CARDS} cards with filters, as many columns as fit, and a Cards/List switch.`}
      >
        <Grid workers={gridWorkers} now={now} />
        <div className="gallery__row">
          <Variant name="grid-loading" caption="Loading" wide>
            <CardGrid
              label="Loading cards"
              items={[]}
              getKey={() => ""}
              renderCard={() => null}
              renderRow={() => null}
              view={view}
              onViewChange={setView}
              state="loading"
            />
          </Variant>
          <Variant name="grid-error" caption="Couldn't load" wide>
            <CardGrid
              label="Cards that failed"
              items={[]}
              getKey={() => ""}
              renderCard={() => null}
              renderRow={() => null}
              view="cards"
              onViewChange={() => undefined}
              state="error"
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="grid-list" caption="As a list" wide>
            <CardGrid
              label="Worker list"
              items={gridWorkers.slice(0, 4)}
              getKey={(w) => w.id}
              renderCard={() => null}
              renderRow={(w) => (
                <>
                  <StatusDot status={w.status} label={STATUS_WORDS[w.status]} />
                  <strong>{w.name}</strong>
                  <span className="gallery__muted">{w.project}</span>
                </>
              )}
              view="list"
              onViewChange={() => undefined}
            />
          </Variant>
          <Variant name="grid-empty" caption="No cards yet" wide>
            <CardGrid
              label="No cards"
              items={[]}
              getKey={() => ""}
              renderCard={() => null}
              renderRow={() => null}
              view="cards"
              onViewChange={() => undefined}
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-table`}
        title="Table and filters"
        lead={`${TABLE_ROWS.toLocaleString("en-US")} rows: sort, filter, pick columns, select rows. Only the rows on screen are drawn.`}
      >
        <FilteredTable workers={workers} prefix={prefix} />
        <div className="gallery__row">
          <Variant name="table-loading" caption="Loading" wide>
            <DataTable
              label="Loading table"
              rows={[]}
              columns={smallColumns}
              getRowId={(w) => w.id}
              state="loading"
              height={160}
            />
          </Variant>
          <Variant name="table-error" caption="Couldn't load" wide>
            <DataTable
              label="Table that failed"
              rows={[]}
              columns={smallColumns}
              getRowId={(w) => w.id}
              state="error"
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="table-empty" caption="No records yet" wide>
            <DataTable
              label="Empty table"
              rows={[]}
              columns={smallColumns}
              getRowId={(w) => w.id}
              height={160}
            />
          </Variant>
        </div>
        <div className="gallery__row">
          <Variant name="facets-loading" caption="Filters: loading">
            <FacetPanel
              label="Filters loading"
              state={EMPTY_FACETS}
              onChange={() => undefined}
              groups={[]}
              collapsed={false}
              onCollapsedChange={() => undefined}
              loading
            />
          </Variant>
          <Variant name="facets-error" caption="Filters: couldn't load">
            <FacetPanel
              label="Filters that failed"
              state={EMPTY_FACETS}
              onChange={() => undefined}
              groups={[]}
              collapsed={false}
              onCollapsedChange={() => undefined}
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="facets-empty" caption="Filters: none yet">
            <FacetPanel
              label="No filters"
              state={EMPTY_FACETS}
              onChange={() => undefined}
              groups={[]}
              collapsed={false}
              onCollapsedChange={() => undefined}
            />
          </Variant>
          <Variant name="facets-collapsed" caption="Filters: hidden">
            <FacetPanel
              label="Hidden filters"
              state={EMPTY_FACETS}
              onChange={() => undefined}
              groups={[]}
              collapsed
              onCollapsedChange={() => undefined}
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-split`}
        title="Detail page"
        lead="Properties and switches, a timeline you can scrub, the map, and a table below."
      >
        <SplitSample now={now} prefix={prefix} />
        <div className="gallery__row">
          <Variant name="split-loading" caption="Loading" wide>
            <DetailSplitView label="Worker details" properties={null} state="loading" />
          </Variant>
          <Variant name="split-error" caption="Couldn't load">
            <DetailSplitView
              label="Worker details"
              properties={null}
              state="error"
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="split-empty" caption="Nothing picked">
            <DetailSplitView label="Worker details" properties={null} state="empty" />
          </Variant>
        </div>
        <div className="gallery__row">
          <Variant name="props-loading" caption="Details: loading">
            <PropertyList title="Website Supervisor" items={[]} state="loading" />
          </Variant>
          <Variant name="props-error" caption="Details: couldn't load">
            <PropertyList
              title="Website Supervisor"
              items={[]}
              state="error"
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="props-empty" caption="Details: nothing yet">
            <PropertyList title="Website Supervisor" items={[]} />
          </Variant>
          <Variant name="timeline-loading" caption="Timeline: loading">
            <TimelineScrubber
              from={now - DAY_MS}
              to={now}
              events={[]}
              value="live"
              onChange={() => undefined}
              label="Timeline loading"
              state="loading"
            />
          </Variant>
          <Variant name="timeline-error" caption="Timeline: couldn't load">
            <TimelineScrubber
              from={now - DAY_MS}
              to={now}
              events={[]}
              value="live"
              onChange={() => undefined}
              label="Timeline that failed"
              state="error"
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="timeline-empty" caption="Timeline: no events yet">
            <TimelineScrubber
              from={now - DAY_MS}
              to={now}
              events={[]}
              value="live"
              onChange={() => undefined}
              label="Empty timeline"
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-map`}
        title="Relationship map"
        lead="Who handed work to whom: tiles in their status color, labeled lines, a caption under each."
      >
        <Variant name="map" caption="Handoff chain" wide>
          <TopologyMap label="Handoff chain" nodes={nodes} links={links} />
        </Variant>
        <div className="gallery__row">
          <Variant name="map-empty" caption="Nothing to draw">
            <TopologyMap label="Empty map" nodes={[]} links={[]} />
          </Variant>
          <Variant name="map-loading" caption="Loading">
            <TopologyMap label="Map" nodes={[]} links={[]} state="loading" />
          </Variant>
          <Variant name="map-error" caption="Couldn't load">
            <TopologyMap
              label="Map"
              nodes={[]}
              links={[]}
              state="error"
              error="The Ledger did not answer."
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-notices`}
        title="Notices"
        lead="Advisories and things you must do, each with its button and Dismiss."
      >
        <div className="gallery__stack" data-gallery="notices">
          <Banner
            tone="info"
            title="A new version of Plenipo is ready"
            action={
              <Button size="sm" variant="primary">
                Restart
              </Button>
            }
            onDismiss={() => undefined}
          />
          <Banner
            tone="pending"
            title="Senior Developer is waiting for your approval"
            action={
              <Button size="sm" variant="primary">
                Review
              </Button>
            }
          >
            git push origin main
          </Banner>
          <Banner
            tone="warn"
            title="Claude Code reached its usage limit"
            onDismiss={() => undefined}
          >
            Workers that use it wait until the limit resets at 3:15 PM.
          </Banner>
          {!dismissed && (
            <Banner
              tone="error"
              title="The Ledger failed its integrity check"
              role="status"
              onDismiss={() => setDismissed(true)}
            >
              Plenipo made a backup before opening it.
            </Banner>
          )}
          <Banner tone="ok" title="All checks passed" />
        </div>
      </Section>

      <Section id={`${prefix}-controls`} title="Controls">
        <div className="gallery__row">
          <Variant name="buttons" caption="Buttons">
            <Button variant="primary">Give objective</Button>
            <Button>Cancel</Button>
            <Button variant="quiet" icon="refresh">
              Refresh
            </Button>
            <Button variant="danger">Stop all</Button>
            <Button size="sm" variant="primary">
              Small
            </Button>
            <Button disabled>Disabled</Button>
            <IconButton icon="more" label="More actions" />
          </Variant>
          <Variant name="choices" caption="Switches, checkboxes, and choices">
            <Switch label="Remote computers (SSH)" checked={on} onChange={setOn} />
            <Checkbox label="Online" count={7} checked={checked} onChange={setChecked} />
            <Segmented
              label="Choice"
              value={choice}
              onChange={setChoice}
              options={[
                { value: "a", label: "Cards" },
                { value: "b", label: "List" },
              ]}
            />
          </Variant>
          <Variant name="fields" caption="Fields">
            <SearchField value={search} onChange={setSearch} placeholder="Search workers" />
            <TextField
              label="Project name"
              value={text}
              onChange={setText}
              hint="Shown on every page."
            />
            <Select
              label="When to ask you"
              value="changes"
              onChange={() => undefined}
              options={[
                { value: "every", label: "Every command" },
                { value: "changes", label: "Anything that changes" },
                { value: "allowed", label: "Only as allowed" },
              ]}
            />
          </Variant>
        </div>
        <Variant name="tabs" caption="Tabs" wide>
          <Tabs
            label="Sample tabs"
            value={tab}
            onChange={setTab}
            idPrefix={`${prefix}-tabs`}
            tabs={[
              { value: "one", label: "Your terminal" },
              {
                value: "two",
                label: "Senior Developer",
                badge: <CountBadge count={2} label="new lines" />,
              },
              { value: "three", label: "Shop server" },
            ]}
          />
          <div
            role="tabpanel"
            id={`${prefix}-tabs-panel-${tab}`}
            aria-labelledby={`${prefix}-tabs-tab-${tab}`}
            className="gallery__muted"
          >
            {tab === "one"
              ? "Your terminal"
              : tab === "two"
                ? "Senior Developer's commands"
                : "Shop server"}
          </div>
        </Variant>
        <div className="gallery__row">
          <Variant name="tabs-closable" caption="Tabs you can close (the terminal)">
            <Tabs
              label="Terminals"
              value={terminal}
              onChange={setTerminal}
              tabs={terminals.map((t) => ({
                value: t,
                label:
                  t === "pc" ? (
                    "This PC"
                  ) : t === "shop" ? (
                    <>
                      <StatusDot status="error" label="Production" />
                      Shop
                    </>
                  ) : (
                    "Operations Engineer · Dev box"
                  ),
                onClose: () => setTerminals((all) => all.filter((x) => x !== t)),
                closeLabel: `Close ${t}`,
              }))}
            />
          </Variant>
          <Variant name="tabs-vertical" caption="Sections down the side (Settings)">
            <Tabs
              label="Sample sections"
              orientation="vertical"
              value={section}
              onChange={setSection}
              tabs={[
                { value: "tools", label: "AI tools" },
                { value: "servers", label: "Servers" },
                { value: "about", label: "About Plenipo" },
              ]}
            />
          </Variant>
        </div>
        <div className="gallery__row">
          <Variant name="menu" caption="A menu behind a button">
            <MenuButton
              label="New terminal"
              icon="plus"
              onSelect={() => undefined}
              items={[{ id: "pc", label: "This PC", icon: "terminal" }]}
            />
          </Variant>
          <Variant name="menu-open" caption="The menu, open">
            <div className="gallery__menu-room">
              <MenuButton
                label="New terminal"
                icon="plus"
                defaultOpen
                onSelect={() => undefined}
                items={[
                  { id: "pc", label: "This PC", icon: "terminal" },
                  { id: "shop", label: "Shop", icon: "server", hint: "PRODUCTION" },
                  {
                    id: "old",
                    label: "Old box (its ID is not pinned)",
                    icon: "server",
                    disabled: true,
                  },
                ]}
              />
            </div>
          </Variant>
          <Variant name="menu-empty" caption="The menu, with nothing in it">
            <div className="gallery__menu-room">
              <MenuButton
                label="Servers"
                defaultOpen
                onSelect={() => undefined}
                items={[]}
                empty="No servers yet: add one in Settings → Servers"
              />
            </div>
          </Variant>
          <Variant name="resize" caption="An edge to drag (or move with the arrow keys)">
            <div className="gallery__resize">
              <div className="gallery__muted">The page</div>
              <ResizeHandle
                label="Resize the sample panel"
                value={panelSize}
                min={80}
                max={240}
                edge="top"
                onChange={setPanelSize}
              />
              <div className="gallery__muted">A panel, {panelSize} px tall</div>
            </div>
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-terminal`}
        title="The terminal"
        lead="The colors programs print in the terminal, on its background, in this theme."
      >
        <Variant name="terminal-colors" caption="Terminal colors" wide>
          <TerminalSample />
        </Variant>
        <div className="gallery__row">
          <Variant name="log" caption="A worker's commands, read-only (a watch tab)">
            <LogView
              label="Operations Engineer on Shop"
              className="gallery__log"
              lines={[
                { text: "$ systemctl status nginx", tone: "command" },
                { text: "● nginx.service - nginx" },
                { text: "   Active: active (running)" },
                { text: "Finished after 0.4 s", tone: "ok" },
                { text: "$ systemctl restart nginx", tone: "command" },
                { text: "Job for nginx.service failed.", tone: "error" },
                { text: "Refused: run rm -rf /srv on Shop — never run", tone: "warn" },
                { text: "Disconnected: you disconnected the worker", tone: "muted" },
              ]}
            />
          </Variant>
          <Variant name="log-empty" caption="A watch tab before the first command">
            <LogView
              label="Operations Engineer on Shop"
              className="gallery__log"
              lines={[]}
              empty="Connected. Commands appear here as the worker runs them."
            />
          </Variant>
        </div>
      </Section>

      <Section id={`${prefix}-states`} title="Empty, loading, and error">
        <div className="gallery__row">
          <Variant name="empty" caption="Nothing yet">
            <EmptyState
              title="No workers yet"
              action={
                <Button size="sm" variant="primary">
                  Hire a worker
                </Button>
              }
            >
              Workers appear here when a Supervisor brings them in.
            </EmptyState>
          </Variant>
          <Variant name="loading" caption="Loading">
            <LoadingState label="Loading workers" />
          </Variant>
          <Variant name="error" caption="Couldn't load">
            <ErrorState
              title="Couldn't load workers"
              message="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-brand`}
        title="Plenipo and Pip"
        lead="The owner's approved brand kit: the P, the logo, and Pip, Plenipo's robot, in all 15 poses."
      >
        <div className="gallery__row">
          <Variant name="brand-mark" caption="The P (the left strip)">
            <div className="gallery__brand-row">
              <PlenipoMark size={16} />
              <PlenipoMark size={28} />
              <PlenipoMark size={48} label="Plenipo" />
            </div>
          </Variant>
          <Variant name="brand-logo" caption="The logo, with Pip on the n">
            <PlenipoLogo height={56} />
          </Variant>
          <Variant name="brand-square" caption="The square logo">
            <PlenipoLogo variant="square" height={120} />
          </Variant>
        </div>
        <Variant name="pip-poses" caption="Pip's poses, and where each is used" wide>
          <div className="gallery__pips">
            {PIP_POSES.map((p) => (
              <figure key={p.pose} className="gallery__pip">
                <Pip pose={p.pose} size="sm" />
                <figcaption>
                  <strong>{p.doing}</strong>
                  <span className="gallery__muted">{p.use}</span>
                </figcaption>
              </figure>
            ))}
          </div>
        </Variant>
        <div className="gallery__row">
          <Variant name="empty-pip" caption="Nothing yet, with Pip">
            <EmptyState pip="recharging" title="All quiet">
              Nothing is waiting for you.
            </EmptyState>
          </Variant>
          <Variant name="error-pip" caption="Couldn't load, with Pip">
            <ErrorState
              pip="support"
              title="Couldn't load the terminal"
              message="The AI tool did not answer."
              onRetry={() => undefined}
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-tokens`}
        title="Colors, type, and spacing"
        lead="The design tokens. Every color on every screen comes from this list."
      >
        <div className="gallery__swatches">
          {(Object.keys(COLOR_ROLES) as ColorToken[]).map((t) => (
            <Swatch key={t} token={t} />
          ))}
        </div>
        <div className="gallery__row">
          <Variant name="type-scale" caption="Text sizes">
            {Object.entries(FONT_SIZE).map(([k, v]) => (
              <div key={k} style={{ fontSize: `var(--ui-font-${k})` }}>
                {k} · {v}px · <span className="ui-num">1,234.50</span>
              </div>
            ))}
          </Variant>
          <Variant name="spacing" caption="Spacing">
            {Object.entries(SPACE).map(([k, v]) => (
              <div key={k} className="gallery__space">
                <span className="gallery__bar" style={{ width: `var(--ui-space-${k})` }} />
                <span className="ui-num">
                  space-{k} · {v}px
                </span>
              </div>
            ))}
          </Variant>
          <Variant name="radius" caption="Corners">
            {Object.entries(RADIUS).map(([k, v]) => (
              <span
                key={k}
                className="gallery__radius"
                style={{ borderRadius: `var(--ui-radius-${k})` }}
              >
                {k} {v === 999 ? "" : `${v}px`}
              </span>
            ))}
          </Variant>
          <Variant name="icons" caption="Icons">
            <div className="gallery__icons">
              {ICON_NAMES.map((n) => (
                <span key={n} title={n}>
                  <Icon name={n} label={n} />
                </span>
              ))}
            </div>
          </Variant>
        </div>
      </Section>
    </div>
  );
}

export type GalleryLayout = "one" | "both";

export function Gallery({
  now: fixedNow,
  live,
  theme,
}: {
  /** The moment the samples end at (tests fix it; the app leaves it to the clock). */
  now?: number;
  live?: GalleryLive;
  /** The app's current theme (the one-theme layout follows it). */
  theme: ThemeName;
}) {
  const [layout, setLayout] = useState<GalleryLayout>("one");
  const [openedAt] = useState(() => Date.now());
  const now = fixedNow ?? openedAt;
  return (
    <div className="gallery">
      <div className="gallery__toolbar">
        <p className="gallery__lead">
          Every building block, in every state. Switch light and dark in the top bar, or show both.
        </p>
        <Segmented
          label="Themes"
          value={layout}
          onChange={setLayout}
          options={[
            { value: "one", label: theme === "dark" ? "Dark" : "Light" },
            { value: "both", label: "Both side by side" },
          ]}
        />
      </div>
      {layout === "one" ? (
        <GalleryBody now={now} prefix="g" {...(live ? { live } : {})} />
      ) : (
        <div className="gallery__both">
          <div data-theme="dark" className="gallery__pane" aria-label="Dark theme" role="group">
            <GalleryBody now={now} prefix="gd" {...(live ? { live } : {})} />
          </div>
          <div data-theme="light" className="gallery__pane" aria-label="Light theme" role="group">
            <GalleryBody now={now} prefix="gl" {...(live ? { live } : {})} />
          </div>
        </div>
      )}
    </div>
  );
}
