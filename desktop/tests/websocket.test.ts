import { describe, expect, it } from "vitest";

import {
  backoffDelayMs,
  CHAMP_SELECT_TOPIC,
  parseWampEvent,
} from "../src/main/lcu/websocket";

describe("backoffDelayMs (mirrors the Rust backoff tests)", () => {
  it("grows 2s → 4s → 8s → 16s then caps at 30s (+ <1s jitter)", () => {
    expect(backoffDelayMs(0)).toBeGreaterThanOrEqual(2000);
    expect(backoffDelayMs(0)).toBeLessThan(3000);
    expect(backoffDelayMs(1)).toBeGreaterThanOrEqual(4000);
    expect(backoffDelayMs(2)).toBeGreaterThanOrEqual(8000);
    expect(backoffDelayMs(3)).toBeGreaterThanOrEqual(16000);
    const capped = backoffDelayMs(10);
    expect(capped).toBeGreaterThanOrEqual(30_000);
    expect(capped).toBeLessThan(31_000);
  });

  it("strictly increases with consecutive errors below the cap", () => {
    const d0 = backoffDelayMs(0);
    const d1 = backoffDelayMs(1);
    const d2 = backoffDelayMs(2);
    expect(d1).toBeGreaterThan(d0);
    expect(d2).toBeGreaterThan(d1);
  });
});

describe("parseWampEvent", () => {
  it("parses a WAMP EVENT (type 8) with data", () => {
    const event = parseWampEvent(
      JSON.stringify([8, CHAMP_SELECT_TOPIC, { data: { myCellId: 2 } }]),
    );
    expect(event).toEqual({
      topic: CHAMP_SELECT_TOPIC,
      data: { myCellId: 2 },
    });
  });

  it("maps a null/absent data payload to null (session cleared)", () => {
    expect(
      parseWampEvent(JSON.stringify([8, CHAMP_SELECT_TOPIC, { data: null }]))?.data,
    ).toBeNull();
    expect(
      parseWampEvent(JSON.stringify([8, CHAMP_SELECT_TOPIC, {}]))?.data,
    ).toBeNull();
  });

  it("ignores non-EVENT frames and junk", () => {
    expect(parseWampEvent(JSON.stringify([5, CHAMP_SELECT_TOPIC]))).toBeNull();
    expect(parseWampEvent(JSON.stringify({ hello: 1 }))).toBeNull();
    expect(parseWampEvent("{not json")).toBeNull();
  });
});
