import { Button } from "@plenipo/ui";

import { startSetupTour, useSetupTourLabel, useSetupTourRunning } from "./store";

/** Starts the setup tour, or picks it up where you stopped (Home and Settings → Organization). */
export function SetupTourButton({ size }: { size?: "sm" }) {
  const label = useSetupTourLabel();
  const running = useSetupTourRunning();
  return (
    <Button
      variant="quiet"
      {...(size ? { size } : {})}
      disabled={running}
      title={running ? "The setup tour is showing" : undefined}
      onClick={startSetupTour}
    >
      {label}
    </Button>
  );
}
