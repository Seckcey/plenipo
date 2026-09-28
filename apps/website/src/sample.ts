// Entirely fictional, authored examples. This module has no app or provider imports.
export const people = [
  {
    id: "company",
    name: "Your organization",
    rank: "You set the direction",
    tool: "Your decisions",
    icon: "organization",
    parent: null,
    x: 0,
    y: 154,
    objective: "Build a helpful website for a new business.",
    message: "Make our services easy to understand. Bring the finished work back for my review.",
  },
  {
    id: "manager",
    name: "Development Manager",
    rank: "Manager",
    tool: "Claude Code",
    icon: "briefcase",
    parent: "company",
    x: 268,
    y: 154,
    objective: "Turn the website goal into a clear plan.",
    message:
      "I’ve split the work into a page draft, a review, and clear supporting copy. The supervisor will keep the pieces together.",
  },
  {
    id: "supervisor",
    name: "Website Supervisor",
    rank: "Supervisor",
    tool: "Grok",
    icon: "flag",
    parent: "manager",
    x: 536,
    y: 154,
    objective: "Bring the website team’s work together.",
    message:
      "The page draft is ready. I’m asking the reviewer to check the links and layout before we send it back to you.",
  },
  {
    id: "developer",
    name: "Senior Developer",
    rank: "Worker",
    tool: "Claude Code",
    icon: "code",
    parent: "supervisor",
    x: 804,
    y: 0,
    objective: "Build a clear, welcoming services page.",
    message:
      "The sample page is built. It includes the services, a simple navigation menu, and space for your contact details.",
  },
  {
    id: "reviewer",
    name: "Code Reviewer",
    rank: "Worker",
    tool: "Codex",
    icon: "check",
    parent: "supervisor",
    x: 804,
    y: 154,
    objective: "Check the page before it goes for approval.",
    message:
      "The sample checks are complete. The links work, the headings read clearly, and the layout fits a small screen.",
  },
  {
    id: "writer",
    name: "Documentation Writer",
    rank: "Worker",
    tool: "Kimi",
    icon: "document",
    parent: "supervisor",
    x: 804,
    y: 308,
    objective: "Explain what changed and how to review it.",
    message:
      "I’ve prepared the handoff notes: what changed, what was checked, and what still needs your decision.",
  },
] as const;
export type Person = (typeof people)[number];
export type PersonId = Person["id"];
export const stories = [
  {
    id: "team",
    label: "Meet your team",
    title: "A place for every role.",
    description:
      "Drag the cards. Follow the reporting lines. Pick someone to see their part in the work.",
    pip: "03-presenting",
    tip: "Try moving a card. The blue lines keep the team connected. Positions change here; reporting relationships stay the same.",
  },
  {
    id: "conversation",
    label: "Follow the work",
    title: "The context stays with the work.",
    description: "Open a sample conversation without losing sight of the team around it.",
    pip: "14-support",
    tip: "Pick a team member, then switch between the page draft and handoff conversations. These are written examples, not live AI replies.",
  },
  {
    id: "approval",
    label: "Make the call",
    title: "Your decision. A clear next step.",
    description: "Try a sample approval and see how the team’s next step changes.",
    pip: "04-thinking",
    tip: "You’re in control. Approve the sample or send it back for changes. Nothing is sent, run, or published.",
  },
  {
    id: "activity",
    label: "See the record",
    title: "Know how you got here.",
    description: "Follow a short sample record from the first objective to the final handoff.",
    pip: "06-quality-check",
    tip: "A clear record helps you see who did what. Pick an entry to meet the team member behind it.",
  },
] as const;
export type StoryId = (typeof stories)[number]["id"];
export type Decision = "waiting" | "approved" | "changes";
export const activity = [
  { time: "09:00", person: "company", text: "You set the website objective." },
  { time: "09:02", person: "manager", text: "Development Manager made a plan." },
  { time: "09:05", person: "supervisor", text: "Website Supervisor shared the tasks." },
  { time: "09:18", person: "developer", text: "Senior Developer prepared the page draft." },
  { time: "09:24", person: "reviewer", text: "Code Reviewer completed the sample checks." },
  { time: "09:28", person: "writer", text: "Documentation Writer prepared the handoff." },
] as const;
