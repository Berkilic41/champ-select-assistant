// LCU lockfile discovery + parsing — straight port of src-tauri/src/lcu/lockfile.rs.
// The lockfile is written by the League Client as `name:pid:port:password:protocol`
// and disappears when the client closes.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

export interface Lockfile {
  name: string;
  pid: number;
  port: number;
  password: string;
  protocol: string;
}

export class LockfileError extends Error {}

/** Parse the raw lockfile content (`name:pid:port:password:protocol`). */
export function parseLockfile(content: string): Lockfile {
  const parts = content.trim().split(":");
  if (parts.length !== 5) {
    throw new LockfileError(
      `lockfile format geçersiz: 5 part beklendi, ${parts.length} bulundu`,
    );
  }
  const pid = Number(parts[1]);
  const port = Number(parts[2]);
  if (!Number.isInteger(pid)) throw new LockfileError("PID parse hatası");
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new LockfileError("port parse hatası");
  }
  return { name: parts[0], pid, port, password: parts[3], protocol: parts[4] };
}

/** The four candidate install locations probed in order (mirrors the Rust list). */
export function lockfileCandidates(
  env: NodeJS.ProcessEnv = process.env,
): string[] {
  const paths = [
    "C:\\Riot Games\\League of Legends\\lockfile",
    "C:\\Program Files\\Riot Games\\League of Legends\\lockfile",
    "C:\\Program Files (x86)\\Riot Games\\League of Legends\\lockfile",
  ];
  const home = env.USERPROFILE;
  if (home) {
    paths.push(join(home, "Riot Games", "League of Legends", "lockfile"));
  }
  return paths;
}

/**
 * Find and parse the lockfile, probing each candidate path in order.
 * Throws `LockfileError` (with the probed paths) when the client isn't running.
 */
export function findLockfile(
  candidates: string[] = lockfileCandidates(),
): Lockfile {
  for (const path of candidates) {
    if (existsSync(path)) {
      return parseLockfile(readFileSync(path, "utf8"));
    }
  }
  throw new LockfileError(
    "League Client lockfile bulunamadı. League of Legends açık mı? Denenen pathler:\n" +
      candidates.map((p) => `  - ${p}`).join("\n"),
  );
}
