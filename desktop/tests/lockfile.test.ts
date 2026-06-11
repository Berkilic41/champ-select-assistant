import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import {
  findLockfile,
  LockfileError,
  lockfileCandidates,
  parseLockfile,
} from "../src/main/lcu/lockfile";

describe("parseLockfile", () => {
  it("parses a valid lockfile (mirrors the Rust test)", () => {
    const lf = parseLockfile("LeagueClient:12345:50123:testpassword:https");
    expect(lf).toEqual({
      name: "LeagueClient",
      pid: 12345,
      port: 50123,
      password: "testpassword",
      protocol: "https",
    });
  });

  it("rejects malformed content", () => {
    expect(() => parseLockfile("invalid")).toThrow(LockfileError);
  });

  it("rejects a non-numeric port", () => {
    expect(() => parseLockfile("a:1:notaport:pw:https")).toThrow(LockfileError);
  });

  it("trims trailing whitespace/newline", () => {
    const lf = parseLockfile("LeagueClient:1:2:pw:https\n");
    expect(lf.port).toBe(2);
  });
});

describe("lockfileCandidates", () => {
  it("probes the three machine-wide paths plus USERPROFILE", () => {
    const paths = lockfileCandidates({ USERPROFILE: "C:\\Users\\test" });
    expect(paths).toHaveLength(4);
    expect(paths[0]).toBe("C:\\Riot Games\\League of Legends\\lockfile");
    expect(paths[3]).toBe(
      join("C:\\Users\\test", "Riot Games", "League of Legends", "lockfile"),
    );
  });

  it("omits the home path when USERPROFILE is unset", () => {
    expect(lockfileCandidates({})).toHaveLength(3);
  });
});

describe("findLockfile", () => {
  let dir: string | undefined;
  afterEach(() => {
    if (dir) rmSync(dir, { recursive: true, force: true });
  });

  it("reads the first existing candidate", () => {
    dir = mkdtempSync(join(tmpdir(), "csa-lockfile-"));
    const path = join(dir, "lockfile");
    writeFileSync(path, "LeagueClient:9:50000:pw:https");
    const lf = findLockfile([join(dir, "missing"), path]);
    expect(lf.port).toBe(50000);
  });

  it("throws with the probed paths when nothing exists", () => {
    expect(() => findLockfile(["X:\\does\\not\\exist\\lockfile"])).toThrow(
      /League of Legends açık mı/,
    );
  });
});
