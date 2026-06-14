# Security Policy — Champ Select Assistant

## Supported versions

The project is in public beta. Only the **latest released version** receives
security fixes; please reproduce any issue on the newest build before reporting.

| Version            | Supported |
|--------------------|-----------|
| Latest `0.10.x` beta | ✅        |
| Older prereleases  | ❌        |

## Reporting a vulnerability

**Do not open a public issue for a security vulnerability.**

- Preferred: open a private **GitHub Security Advisory** at
  <https://github.com/Berkilic41/champ-select-assistant/security/advisories/new>.
- Alternatively, email the maintainer (see the GitHub profile) with the subject
  `SECURITY: champ-select-assistant`.

Please include: affected version, reproduction steps, impact, and any logs
(redact secrets — never paste an API key). We aim to acknowledge within a few
days and to ship a fix in the next beta where feasible.

## Scope & threat model

This is a **local desktop app** (Electron host + Rust/WASM engine). Most data
stays on the user's machine; see [PRIVACY.md](PRIVACY.md) for the exact network
surface. Relevant security properties:

- **No automation of gameplay.** The app never auto-picks, auto-bans, or
  auto-locks champions — it only *suggests*. This is intentional, both for
  security and to respect the Riot API / League of Legends Terms of Service.
- **LCU-first, no developer key required.** Normal use reads the local League
  Client (LCU) only. An optional Riot API key, if provided, lives solely in a
  local `.env` (gitignored) — it is never bundled into the binary or logged.
- **The personal Riot key is server-side only** for the data backend
  (Cloudflare Worker secret); the desktop client never holds it. The manual
  ingestion trigger is gated by a bearer secret.
- **Renderer hardening.** `contextIsolation` + `sandbox` on, no remote
  navigation/child windows, and IPC handlers reject any non-renderer sender.
- **No telemetry.** No analytics or tracking. Optional anonymized feedback
  upload is **off by default** and PII-gated (see PRIVACY.md).

## Out of scope

- Issues that require a user to run a malicious local `.env` or to install
  tampered binaries from outside the official releases.
- Rate-limit / abuse of third-party services (Riot, Cloudflare) using your own
  credentials.

## Releases & signing

Windows installers are currently **unsigned** (no code-signing certificate yet);
this is disclosed in the release notes. Auto-update metadata (`latest.yml`) is
published alongside each release. Verify you downloaded from the official
GitHub Releases page.

---

Champ Select Assistant isn't endorsed by Riot Games.
