import { afterEach, beforeEach } from "vitest";
import { cleanup } from "@testing-library/react";

// The startup welcome screen is covered by its own test; keep it out of the other App tests.
beforeEach(() => localStorage.setItem("rpglib.welcome", "off"));
afterEach(cleanup);
