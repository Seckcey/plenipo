import { memo, useCallback, useEffect, useRef, useState } from "react";
import {
  Background,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  useNodesState,
} from "@xyflow/react";
import type { FitViewOptions, Node, NodeProps, ReactFlowInstance } from "@xyflow/react";
import { activity, people, stories } from "./sample";
import type { Decision, Person, PersonId, StoryId } from "./sample";
import "@xyflow/react/dist/style.css";
import "./demo.css";

const paths: Record<string, string> = {
  organization: "M8 3h8v5H8zM3 16h6v5H3zM15 16h6v5h-6zM12 8v4M6 16v-4h12v4",
  briefcase: "M8 7V4h8v3M3 7h18v13H3zM3 12h18M10 10v4h4v-4",
  flag: "M5 21V3h14l-4 5 4 5H5",
  code: "m8 6-6 6 6 6m8-12 6 6-6 6m-3-15-2 18",
  check: "m8 11 3 3 5-6M20 11a9 9 0 1 1-4-7m1 14 5 5",
  document: "M5 2h9l5 5v15H5zM14 2v6h5M8 12h8M8 16h8",
};
function Icon({ name }: { name: string }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d={paths[name] ?? paths.organization} />
    </svg>
  );
}
type TeamNode = Node<{ person: Person }, "team">;
const TeamCard = memo(function TeamCard({ data, selected }: NodeProps<TeamNode>) {
  const person = data.person;
  return (
    <div
      className={`demo-card ${person.id === "company" ? "demo-card-company" : ""} ${selected ? "is-selected" : ""}`}
    >
      {person.parent && <Handle type="target" position={Position.Left} isConnectable={false} />}
      <span className="demo-role-icon">
        <Icon name={person.icon} />
      </span>
      <div className="demo-card-copy">
        <strong>{person.name}</strong>
        <span>{person.rank}</span>
        <div className="demo-card-meta">
          <span className="demo-status">{person.id === "company" ? "You decide" : "Ready"}</span>
          <span>{person.tool}</span>
        </div>
      </div>
      <Handle type="source" position={Position.Right} isConnectable={false} />
    </div>
  );
});
const nodeTypes = { team: TeamCard };
const initialNodes: TeamNode[] = people.map((person) => ({
  id: person.id,
  type: "team",
  position: { x: person.x, y: person.y },
  data: { person },
  ariaLabel: `${person.name}, ${person.rank}. Select to read details.`,
  deletable: false,
}));
const edges = people
  .filter((person) => person.parent)
  .map((person) => ({
    id: `reports-${person.id}`,
    source: person.parent as string,
    target: person.id,
    type: "smoothstep",
    selectable: false,
    deletable: false,
  }));
const fitOptions = {
  padding: { top: "65px", bottom: "140px", left: "30px", right: "30px" },
  maxZoom: 1,
} satisfies FitViewOptions<TeamNode>;
const getPerson = (id: PersonId) => people.find((person) => person.id === id) ?? people[0];

export default function Demo() {
  const [story, setStory] = useState<StoryId>("team");
  const [selected, setSelected] = useState<PersonId | null>(null);
  const [view, setView] = useState<"map" | "list">(() =>
    window.matchMedia("(max-width: 700px), (pointer: coarse)").matches ? "list" : "map",
  );
  const [dark, setDark] = useState(false);
  const [tips, setTips] = useState(true);
  const [moved, setMoved] = useState(false);
  const [decision, setDecision] = useState<Decision>("waiting");
  const [session, setSession] = useState<"draft" | "handoff">("draft");
  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const flow = useRef<ReactFlowInstance<TeamNode> | null>(null);
  const mapArea = useRef<HTMLDivElement | null>(null);
  const inspector = useRef<HTMLElement | null>(null);
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const active = stories.find((item) => item.id === story) ?? stories[0];
  const person = selected ? getPerson(selected) : null;
  // Refit only when the canvas changes size, never during a card drag or pan.
  useEffect(() => {
    const element = mapArea.current;
    if (!element || view !== "map") return;
    const observer = new ResizeObserver(() => {
      void flow.current?.fitView(fitOptions);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [view]);
  const select = useCallback(
    (id: PersonId) => {
      setSelected(id);
      setStory((current) =>
        current === "approval" || current === "activity" ? "conversation" : current,
      );
      setSession("draft");
      setNodes((current) => current.map((node) => ({ ...node, selected: node.id === id })));
      requestAnimationFrame(() => {
        if (window.matchMedia("(max-width: 700px)").matches)
          inspector.current?.scrollIntoView({ block: "start", behavior: "instant" });
      });
    },
    [setNodes],
  );
  const changeStory = (id: StoryId) => {
    setStory(id);
    if (id === "conversation") select("supervisor");
    else {
      setSelected(null);
      setNodes((current) => current.map((node) => ({ ...node, selected: false })));
    }
  };
  const reset = () => {
    setNodes(initialNodes);
    setSelected(null);
    setDecision("waiting");
    setSession("draft");
    setMoved(false);
    requestAnimationFrame(() => {
      void flow.current?.fitView(fitOptions);
    });
  };
  return (
    <div className={`interactive-demo ${dark ? "demo-dark" : ""}`}>
      <div className="demo-stories" role="tablist" aria-label="Explore Plenipo">
        {stories.map((item, index) => (
          <button
            key={item.id}
            ref={(element) => {
              tabRefs.current[index] = element;
            }}
            id={`demo-tab-${item.id}`}
            role="tab"
            aria-label={item.label}
            aria-selected={story === item.id}
            aria-controls="demo-story"
            tabIndex={story === item.id ? 0 : -1}
            onClick={() => changeStory(item.id)}
            onKeyDown={(event) => {
              const next =
                event.key === "ArrowRight"
                  ? (index + 1) % stories.length
                  : event.key === "ArrowLeft"
                    ? (index + stories.length - 1) % stories.length
                    : event.key === "Home"
                      ? 0
                      : event.key === "End"
                        ? stories.length - 1
                        : null;
              if (next !== null) {
                event.preventDefault();
                const item = stories[next];
                if (item) changeStory(item.id);
                tabRefs.current[next]?.focus();
              }
            }}
          >
            <span className="demo-story-number" aria-hidden="true">
              0{index + 1}
            </span>
            {item.label}
          </button>
        ))}
      </div>
      <div id="demo-story" role="tabpanel" aria-labelledby={`demo-tab-${story}`}>
        <div className="demo-story-copy">
          <h2>{active.title}</h2>
          <p>
            {view === "list" && story === "team"
              ? "Pick someone to see their role, objective, and a sample conversation."
              : active.description}
          </p>
        </div>
        <div className="demo-window">
          <div className="demo-window-bar">
            <img
              src={`/brand/plenipo-horizontal-on-${dark ? "dark" : "light"}.webp`}
              width="600"
              height="239"
              alt="Plenipo"
            />
            <span>
              Organization <span aria-hidden="true">/</span> Website team
            </span>
            <button onClick={() => setDark(!dark)} aria-pressed={dark}>
              {dark ? "Light" : "Dark"} preview
            </button>
          </div>
          <div className="demo-toolbar">
            <div>
              <strong>Your organization</strong>
              <span>
                1 department <b>·</b> 1 project <b>·</b> 5 team members
              </span>
            </div>
            <div className="demo-toolbar-actions">
              <div className="demo-switch" aria-label="Team view">
                <button aria-pressed={view === "map"} onClick={() => setView("map")}>
                  Map
                </button>
                <button aria-pressed={view === "list"} onClick={() => setView("list")}>
                  List
                </button>
              </div>
              <button onClick={reset} className="demo-reset">
                Reset demo
              </button>
            </div>
          </div>
          <div
            className={`demo-workspace ${person || story === "approval" || story === "activity" ? "has-details" : ""}`}
          >
            <div className="demo-map-area" ref={mapArea}>
              {view === "map" ? (
                <>
                  <div className="demo-map-label">
                    <span /> Reports to <b>·</b> Drag to explore
                  </div>
                  <ReactFlow<TeamNode>
                    key="map"
                    nodes={nodes}
                    edges={edges}
                    nodeTypes={nodeTypes}
                    onNodesChange={onNodesChange}
                    onInit={(instance) => {
                      flow.current = instance;
                    }}
                    onNodeClick={(_, node) => select(node.data.person.id)}
                    onNodeDragStop={() => setMoved(true)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        const id = (event.target as HTMLElement)
                          .closest(".react-flow__node")
                          ?.getAttribute("data-id");
                        const match = people.find((item) => item.id === id);
                        if (match) {
                          event.preventDefault();
                          select(match.id);
                        }
                      }
                    }}
                    fitView
                    fitViewOptions={fitOptions}
                    minZoom={0.3}
                    maxZoom={1.6}
                    nodesConnectable={false}
                    edgesFocusable={false}
                    deleteKeyCode={null}
                    zoomOnScroll={false}
                    zoomOnDoubleClick={false}
                    preventScrolling={false}
                    colorMode={dark ? "dark" : "light"}
                    aria-label="Sample organization. Select a card with Enter. Arrow keys move focused cards. Use the List button for a simpler view."
                  >
                    <Background gap={24} size={1} />
                    <Controls showInteractive={false} fitViewOptions={fitOptions} />
                    <MiniMap
                      pannable
                      zoomable
                      nodeColor="var(--demo-accent)"
                      nodeBorderRadius={4}
                      maskColor="var(--demo-map-mask)"
                      ariaLabel="Overview of the sample organization"
                    />
                  </ReactFlow>
                  <p className="demo-map-help">
                    Scroll the page freely. Pinch or use + / − to zoom.
                  </p>
                </>
              ) : (
                <div className="demo-team-list" aria-label="Sample team">
                  {people.map((item) => (
                    <button
                      key={item.id}
                      aria-pressed={selected === item.id}
                      onClick={() => select(item.id)}
                    >
                      <span className="demo-role-icon">
                        <Icon name={item.icon} />
                      </span>
                      <span>
                        <strong>{item.name}</strong>
                        <small>
                          {item.rank} · {item.tool}
                        </small>
                        {item.parent && <small>Reports to {getPerson(item.parent).name}</small>}
                      </span>
                      <span aria-hidden="true">↗</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
            {(person || story === "approval" || story === "activity") && (
              <aside className="demo-inspector" aria-label="Sample details" ref={inspector}>
                {story === "approval" ? (
                  <>
                    <div className="demo-inspector-heading">
                      <Icon name="check" />
                      <h3>Your decision</h3>
                    </div>
                    <span className="demo-eyebrow">SAMPLE APPROVAL</span>
                    <h4>Share the website draft</h4>
                    <p>
                      The team has prepared a draft and its review notes. In this example, sharing
                      it waits for you.
                    </p>
                    <dl>
                      <dt>Requested by</dt>
                      <dd>Website Supervisor</dd>
                      <dt>Sample status</dt>
                      <dd aria-live="polite">
                        {decision === "waiting"
                          ? "Waiting for you"
                          : decision === "approved"
                            ? "Approved in this demo"
                            : "Changes requested in this demo"}
                      </dd>
                    </dl>
                    {decision === "waiting" ? (
                      <div className="demo-decision-actions">
                        <button className="demo-primary" onClick={() => setDecision("approved")}>
                          Approve sample
                        </button>
                        <button onClick={() => setDecision("changes")}>Ask for changes</button>
                      </div>
                    ) : (
                      <div className="demo-decision-result">
                        <strong>
                          {decision === "approved"
                            ? "The sample is ready for handoff."
                            : "The sample goes back to the team."}
                        </strong>
                        <p>
                          {decision === "approved"
                            ? "Your choice is now shown in the sample activity record."
                            : "The team would revise the draft before asking again."}
                        </p>
                        <button onClick={() => setDecision("waiting")}>Try the other choice</button>
                      </div>
                    )}
                    <p className="demo-small">
                      This example sends and publishes nothing. Approval rules in the app depend on
                      your settings.
                    </p>
                  </>
                ) : story === "activity" ? (
                  <>
                    <div className="demo-inspector-heading">
                      <Icon name="document" />
                      <h3>A clear record</h3>
                    </div>
                    <p className="demo-small">A fictional morning with your website team.</p>
                    <ol className="demo-timeline">
                      {activity.map((item) => (
                        <li key={item.time}>
                          <button
                            onClick={() => {
                              setStory("conversation");
                              select(item.person);
                            }}
                          >
                            <time>{item.time}</time>
                            <span>{item.text}</span>
                          </button>
                        </li>
                      ))}
                      <li>
                        <time>09:30</time>
                        <span>
                          {decision === "waiting"
                            ? "The sample is waiting for your decision."
                            : decision === "approved"
                              ? "You approved the sample handoff."
                              : "You asked for changes to the sample."}
                        </span>
                      </li>
                    </ol>
                  </>
                ) : (
                  person && (
                    <>
                      <div className="demo-inspector-heading">
                        <h3>{person.name}</h3>
                        <button
                          className="demo-close"
                          aria-label="Close team member details"
                          onClick={() => {
                            setSelected(null);
                            setNodes((current) =>
                              current.map((node) => ({ ...node, selected: false })),
                            );
                          }}
                        >
                          ×
                        </button>
                      </div>
                      <div className="demo-person">
                        <span className="demo-role-icon">
                          <Icon name={person.icon} />
                        </span>
                        <div>
                          <strong>{person.rank}</strong>
                          <span>{person.tool}</span>
                        </div>
                      </div>
                      <div className="demo-objective">
                        <span className="demo-eyebrow">OBJECTIVE</span>
                        <p>{person.objective}</p>
                      </div>
                      <dl>
                        <dt>Reports to</dt>
                        <dd>{person.parent ? getPerson(person.parent).name : "You"}</dd>
                        <dt>Project</dt>
                        <dd>Website</dd>
                      </dl>
                      <h4>Sample conversations</h4>
                      <div className="demo-session-switch">
                        <button
                          aria-pressed={session === "draft"}
                          onClick={() => setSession("draft")}
                        >
                          Page draft
                        </button>
                        <button
                          aria-pressed={session === "handoff"}
                          onClick={() => setSession("handoff")}
                        >
                          Handoff notes
                        </button>
                      </div>
                      <div className="demo-conversation" aria-live="polite">
                        <div>
                          <span>
                            {session === "draft"
                              ? "You · sample message"
                              : "Website Supervisor · sample message"}
                          </span>
                          <p>
                            {session === "draft"
                              ? "Where are we with the website?"
                              : "What should I include in the handoff?"}
                          </p>
                        </div>
                        <div>
                          <span>{person.name} · sample reply</span>
                          <p>
                            {session === "draft"
                              ? person.message
                              : `For ${person.name}, include the objective, the work completed, and the review notes. Keep the final decision with you.`}
                          </p>
                        </div>
                      </div>
                      <p className="demo-small">
                        Written examples. No live conversation or AI connection.
                      </p>
                    </>
                  )
                )}
              </aside>
            )}
          </div>
          <div className="demo-disclosure">
            <span className="demo-status-dot" />
            Interactive sample <span>·</span> No real AI runs. No sign-in. Nothing is saved.
            <a href="#team-panel">
              About the desktop app <span aria-hidden="true">↗</span>
            </a>
          </div>
        </div>
        {tips ? (
          <div className="demo-pip">
            <img
              src={`/brand/pip/pip-${decision === "approved" && story === "approval" ? "05-celebrating" : active.pip}.webp`}
              width="240"
              height="240"
              alt=""
            />
            <div>
              <strong>A little help from Pip</strong>
              <p aria-live="polite">
                {decision === "approved" && story === "approval"
                  ? "That’s your call made! Open “See the record” to find your sample decision. Reset any time to try again."
                  : moved && story === "team"
                    ? "You’ve made room for your team. Select a card to see its objective and sample conversation, or reset to put everything back."
                    : view === "list" && story === "team"
                      ? "Pick a team member to read their objective and sample messages. You can switch to the map any time to explore the reporting lines."
                      : active.tip}
              </p>
            </div>
            <button aria-label="Hide Pip tips" onClick={() => setTips(false)}>
              ×
            </button>
          </div>
        ) : (
          <button className="demo-show-tips" onClick={() => setTips(true)}>
            Show Pip’s tips
          </button>
        )}
      </div>
    </div>
  );
}
