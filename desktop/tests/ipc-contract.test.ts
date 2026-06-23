// V2 — IPC komut-kayıt sözleşmesi. The renderer reaches the host through ONE
// dispatcher (`window.api.invoke("cmd", name, args)`); `ipc.ts` routes `name`
// through a command registry. Per-command tests prove each handler's LOGIC, but
// NOTHING checks that every command the renderer calls is actually registered.
// A typo or a forgotten registration ships as a runtime "komut taşınmadı" error
// no current test catches. This statically pairs the two sides.
//
// Direction asserted: every renderer invoke("name") MUST exist in the registry.
// The reverse (registered-but-unused) is intentionally NOT failed — a typo there
// surfaces as a *missing* name on this side anyway, and aliases / not-yet-wired
// commands are legitimately registered ahead of a caller.

import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

import { describe, expect, it } from "vitest";

import { buildCommandRegistry, type IpcContext } from "../src/main/ipc";

const RENDERER_SRC = resolve(__dirname, "..", "..", "src");

/** Recursively collect renderer .ts/.tsx files. */
function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...sourceFiles(full));
    } else if (/\.tsx?$/.test(entry.name)) {
      out.push(full);
    }
  }
  return out;
}

/** Every command name passed as a string literal to `invoke("name", …)` in the
 *  renderer (optional `<Type>` generic between invoke and the paren). */
function rendererCommandNames(): Set<string> {
  const re = /\binvoke(?:<[^>]*>)?\(\s*['"]([a-z_][a-z0-9_]*)['"]/g;
  const names = new Set<string>();
  for (const file of sourceFiles(RENDERER_SRC)) {
    const text = readFileSync(file, "utf8");
    for (const m of text.matchAll(re)) names.add(m[1]);
  }
  return names;
}

describe("IPC command-registry contract", () => {
  it("registers every command the renderer invokes", () => {
    // buildCommandRegistry only stores closures (never touches ctx during build),
    // so an empty ctx is enough to enumerate the registered command names.
    const registry = buildCommandRegistry({} as unknown as IpcContext);
    const registered = new Set(registry.keys());

    const used = rendererCommandNames();
    // Sanity: the scan must actually find calls (guards against a broken regex/path).
    expect(used.size).toBeGreaterThan(20);

    const missing = [...used].filter((name) => !registered.has(name)).sort();
    expect(missing, `renderer bu komutları çağırıyor ama ipc.ts kaydetmemiş: ${missing.join(", ")}`).toEqual([]);
  });

  it("registers get_app_version and the handler returns the build version", () => {
    // Ayarlar → "Hakkında" sürüm satırı bu komuta dayanır; handler app.getVersion()'ı
    // sarar (electron-stub testte "0.0.0-test" döndürür) ve ctx'e dokunmaz.
    const registry = buildCommandRegistry({} as unknown as IpcContext);
    const handler = registry.get("get_app_version");
    expect(handler).toBeDefined();
    expect(handler!(undefined)).toBe("0.0.0-test");
  });
});
