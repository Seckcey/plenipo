import "@testing-library/jest-dom/vitest";
import { configure } from "@testing-library/react";

// The page's tests meet a stand-in PC through the real lock (Noise, with the browser's own
// cryptography), so a busy machine may take more than the usual second to show what follows.
configure({ asyncUtilTimeout: 8_000 });
