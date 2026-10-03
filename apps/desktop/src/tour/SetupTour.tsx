/**
 * The setup tour (Phase 25, item 2.9; ADR-198): it walks a new owner through signing in to an AI
 * tool, adding a department and a project, hiring the team, choosing their models, and giving the
 * first objective. It points at each part of the screen with Driver.js, moves between pages by
 * itself, and moves on only when a step is really done. It pauses while a dialog is open and
 * remembers where you stopped, in each organization.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import type { OrgSnapshot } from "@plenipo/types";
import { driver, type Driver } from "driver.js";
import "driver.js/dist/driver.css";

import { subscribeLedgerEvents } from "../api/events";
import type { Go } from "../components/views";
import { useOrganization } from "../org/useOrganization";
import { useReducedMotion } from "../org/useLiveView";
import { useOrganizations } from "../orgs/useOrganizations";
import {
  SETUP_STEPS,
  factsOf,
  firstSupervisor,
  nextStep,
  startsByItself,
  type StepId,
} from "./steps";
import {
  endSetupTour,
  progressOf,
  saveProgress,
  setTourOrganization,
  startSetupTour,
  startsByItselfHere,
  useSetupTourRunning,
} from "./store";

const DAY_MS = 24 * 60 * 60 * 1000;
/** It starts by itself a moment after the window opens, once the window has settled. */
const SETTLE_MS = 1000;
/** How often the tour looks at the screen (for its part of the page, and for dialogs). */
const LOOK_MS = 250;
/** How long it waits for its part of the page before pointing at the middle of the screen. */
const WAIT_MS = 1500;
/** Changes to model choices that finish the models step. */
const MODEL_EVENTS = new Set([
  "router.policy_changed",
  "router.rule_changed",
  "router.model_saved",
  "router.models_added",
]);
/** The class on the page while the tour shows: the page stays clickable around its spotlight. */
const ON_CLASS = "setup-tour-on";

/** A test robot drives this window (WebDriver): the tour never starts by itself then. */
const automated = () => typeof navigator !== "undefined" && navigator.webdriver;

const shown = (el: Element) => el.getClientRects().length > 0;

/** The first part of the page with one of these marks that is on screen. */
function findAnchor(names: readonly string[]): Element | null {
  for (const name of names) {
    for (const el of document.querySelectorAll(`[data-tour="${name}"]`)) {
      if (shown(el)) return el;
    }
  }
  return null;
}

/**
 * Driver.js makes everything around its spotlight unclickable and keeps the Tab key inside it.
 * Each step here asks you to do something on the page (signing in happens in a tab at the
 * bottom), so both are undone: the page class that blocks clicks, and its Tab key trap.
 */
function letThePageWork(tour: Driver) {
  document.body.classList.remove("driver-active");
  const events = tour.getState("__events") as
    { onKeydown?: (e: KeyboardEvent) => void } | undefined;
  if (events?.onKeydown) window.removeEventListener("keydown", events.onKeydown, false);
}

/** A dialog is open (the tour waits until it closes). */
const dialogOpen = () =>
  [...document.querySelectorAll('[aria-modal="true"], dialog[open]')].some(
    (d) => !d.closest(".driver-popover") && shown(d),
  );

/**
 * The setup tour for this window: it starts by itself in a new organization, picks up a tour the
 * app closed in the middle of, and shows while `startSetupTour` asked for it.
 */
export function SetupTour({ go, snapshot }: { go: Go; snapshot: OrgSnapshot | null }) {
  const { listing } = useOrganizations();
  const org = listing?.current ?? null;
  const createdAt = listing?.organizations.find((o) => o.id === org)?.createdAt ?? null;
  const running = useSetupTourRunning();
  useEffect(() => setTourOrganization(org), [org]);
  useEffect(() => {
    if (!org || !snapshot || createdAt === null || running) return;
    const progress = progressOf(org);
    if (progress?.status === "going") {
      startSetupTour();
      return;
    }
    if (progress || automated()) return;
    const recent = Date.now() - createdAt < DAY_MS;
    if (!startsByItself(factsOf(snapshot), { seen: false, recent })) return;
    const timer = setTimeout(() => {
      if (!progressOf(org) && startsByItselfHere()) startSetupTour();
    }, SETTLE_MS);
    return () => clearTimeout(timer);
  }, [org, snapshot, createdAt, running]);
  return running && org ? <RunningTour org={org} go={go} /> : null;
}

interface Screen {
  /** A dialog is open: the tour hides until it closes. */
  paused: boolean;
  anchor: Element | null;
  /** Ready to show: its part of the page is there, or it waited long enough. */
  ready: boolean;
}

const sameScreen = (a: Screen, b: Screen) =>
  a.paused === b.paused && a.anchor === b.anchor && a.ready === b.ready;

function RunningTour({ org, go }: { org: string; go: Go }) {
  const { snapshot } = useOrganization();
  const reducedMotion = useReducedMotion();
  /** Where it began: where you stopped (or the start). */
  const [start] = useState(() => {
    const saved = progressOf(org);
    return saved && saved.status !== "finished" ? Math.min(saved.step, SETUP_STEPS.length - 1) : 0;
  });
  /** The step you moved to with Next or Back. */
  const [chosen, setChosen] = useState<number | null>(null);
  /** You went Back to this step: it stays even when it's already done. */
  const [held, setHeld] = useState(false);
  const [modelChanges, setModelChanges] = useState(0);
  const [screen, setScreen] = useState<Screen & { step: StepId | null }>({
    step: null,
    paused: false,
    anchor: null,
    ready: false,
  });
  const facts = useMemo(
    () => (snapshot ? factsOf(snapshot, modelChanges) : null),
    [snapshot, modelChanges],
  );
  // A step that's done moves on by itself, unless you came Back to it.
  const base = chosen ?? start;
  const index = facts ? (held ? base : nextStep(facts, base)) : null;
  const step = index === null ? null : (SETUP_STEPS[index] ?? null);
  const goRef = useRef(go);
  const snapshotRef = useRef(snapshot);
  const stepIdRef = useRef(step?.id);
  useEffect(() => {
    goRef.current = go;
    snapshotRef.current = snapshot;
    stepIdRef.current = step?.id;
  });

  // Model changes count on the models step (a new tour starts from none).
  useEffect(() => {
    let stop: (() => void) | null = null;
    let gone = false;
    subscribeLedgerEvents((event) => {
      if (stepIdRef.current === "models" && MODEL_EVENTS.has(event.eventType)) {
        setModelChanges((n) => n + 1);
      }
    })
      .then((unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      })
      .catch(() => undefined);
    return () => {
      gone = true;
      stop?.();
    };
  }, []);

  // Each new step: remember it, open its page, and bring the supervisor into view.
  useEffect(() => {
    if (index === null) return;
    const now = SETUP_STEPS[index];
    if (!now) return;
    saveProgress(org, { step: index, status: "going" });
    const current = snapshotRef.current;
    const supervisor = now.focusSupervisor && current ? firstSupervisor(current) : null;
    const target = supervisor ? { view: "organization" as const, id: supervisor } : now.page;
    if (target) goRef.current(target);
  }, [index, org]);

  // Look at the screen: dialogs, and the part of the page the step points at.
  useEffect(() => {
    if (!step) return;
    const began = Date.now();
    const look = () => {
      const anchor = findAnchor(step.anchors);
      const next = {
        step: step.id,
        paused: dialogOpen(),
        anchor,
        ready: anchor !== null || step.anchors.length === 0 || Date.now() - began >= WAIT_MS,
      };
      setScreen((was) => (was.step === next.step && sameScreen(was, next) ? was : next));
    };
    const first = setTimeout(look, 0);
    const timer = setInterval(look, LOOK_MS);
    return () => {
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [step]);
  /** What the screen shows for this step (nothing yet while it opens). */
  const here = screen.step === step?.id ? screen : null;

  // Driver.js shows the spotlight and the words; the page stays clickable around it.
  const tour = useRef<Driver | null>(null);
  useEffect(() => {
    document.body.classList.add(ON_CLASS);
    return () => {
      tour.current?.destroy();
      tour.current = null;
      document.body.classList.remove(ON_CLASS);
    };
  }, []);

  const stop = () => {
    if (index !== null) saveProgress(org, { step: index, status: "stopped" });
    endSetupTour();
  };
  const finish = () => {
    saveProgress(org, { step: 0, status: "finished" });
    endSetupTour();
  };
  const last = index === SETUP_STEPS.length - 1;
  const doneNow = !!(step?.done && facts && step.done(facts));
  const nextWords = last ? "Finish" : step?.done && !doneNow ? "Skip this step" : "Next";
  /** What the popover's buttons do (kept current; Driver.js keeps the first functions). */
  const actions = useRef({ stop: () => {}, next: () => {}, back: () => {} });
  useEffect(() => {
    actions.current = {
      stop,
      next: () => {
        if (index === null || !facts) return;
        setHeld(false);
        if (last) finish();
        else setChosen(nextStep(facts, index + 1));
      },
      back: () => {
        if (index === null || index === 0) return;
        setHeld(true);
        setChosen(index - 1);
      },
    };
  });

  useEffect(() => {
    if (!step || index === null || !here || here.paused || !here.ready) {
      if (tour.current?.isActive()) tour.current.destroy();
      return;
    }
    tour.current ??= driver({
      animate: !reducedMotion,
      smoothScroll: !reducedMotion,
      allowClose: true,
      allowKeyboardControl: false,
      overlayClickBehavior: "none",
      overlayOpacity: 0.45,
      stagePadding: 6,
      stageRadius: 10,
      popoverClass: "setup-tour",
    });
    tour.current.highlight({
      ...(here.anchor ? { element: here.anchor } : {}),
      popover: {
        title: step.title,
        description: step.words,
        showButtons: ["next", "previous", "close"],
        disableButtons: index === 0 ? ["previous"] : [],
        nextBtnText: nextWords,
        prevBtnText: "Back",
        closeBtnLabel: "Stop the tour for now",
        showProgress: true,
        progressText: `Step ${index + 1} of ${SETUP_STEPS.length}`,
        onNextClick: () => actions.current.next(),
        onPrevClick: () => actions.current.back(),
        onCloseClick: () => actions.current.stop(),
      },
    });
    letThePageWork(tour.current);
  }, [step, index, here, nextWords, reducedMotion]);

  return null;
}
