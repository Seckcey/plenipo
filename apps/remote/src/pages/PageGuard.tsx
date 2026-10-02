import { Component, type ReactNode } from "react";
import { Button } from "@plenipo/ui";

/**
 * One page's safety net: if a page cannot be shown (an answer it did not expect), it says so, and
 * the rest of the phone's page keeps working.
 */
export class PageGuard extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <section className="page">
        <p className="form-error" role="alert">
          This page could not be shown. Your PC may run another version of Plenipo.
        </p>
        <Button onClick={() => this.setState({ failed: false })}>Try again</Button>
      </section>
    );
  }
}
