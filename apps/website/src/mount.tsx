import { Component, createElement } from "react";
import type { ReactNode } from "react";
import { createRoot } from "react-dom/client";
import Demo from "./Demo";

class DemoBoundary extends Component<
  { children: ReactNode; onFailure: () => void },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  componentDidCatch() {
    this.props.onFailure();
  }
  render() {
    return this.state.failed ? null : this.props.children;
  }
}
export function mountDemo(element: HTMLElement, ready: () => void, fail: () => void) {
  const root = createRoot(element);
  root.render(
    createElement(DemoBoundary, {
      onFailure: fail,
      children: createElement(
        "div",
        {
          ref: (node: HTMLDivElement | null) => {
            if (node) ready();
          },
        },
        createElement(Demo),
      ),
    }),
  );
  return () => root.unmount();
}
