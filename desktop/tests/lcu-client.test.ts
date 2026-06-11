import { describe, expect, it } from "vitest";

import {
  lcuAuthHeader,
  lcuBaseUrl,
  lcuWsUrl,
  riotRootCa,
} from "../src/main/lcu/client";

const LOCKFILE = {
  name: "LeagueClient",
  pid: 1,
  port: 50123,
  password: "testpassword",
  protocol: "https",
};

describe("LCU client plumbing", () => {
  it("builds the riot:<password> basic-auth header", () => {
    const header = lcuAuthHeader(LOCKFILE);
    expect(header).toBe(
      "Basic " + Buffer.from("riot:testpassword").toString("base64"),
    );
  });

  it("builds base + ws URLs from the lockfile", () => {
    expect(lcuBaseUrl(LOCKFILE)).toBe("https://127.0.0.1:50123");
    expect(lcuWsUrl(LOCKFILE)).toBe("wss://127.0.0.1:50123");
  });

  it("loads the pinned Riot root CA (never rejectUnauthorized:false)", () => {
    const pem = riotRootCa();
    expect(pem).toContain("-----BEGIN CERTIFICATE-----");
    expect(pem).toContain("-----END CERTIFICATE-----");
  });
});
