# Web UI: 100% functional parity plan

Status: **planned; parity is not complete.**

Created: 2026-11-12. Reference: the desktop application in this fork, including its
current working-tree changes. HEAD at planning time: `197c0016`; the redesign and
recent admission/confirmation work are not all committed in that reference.

## 1. Objective and scope

Expose every desktop capability through the Web UI, or provide a documented,
equivalent browser/server workflow, without requiring a desktop session on the
download server. Preserve the desktop engine, behavior and existing safeguards.

**The user-approved exclusion is shutdown after downloads finish.** Do not add
host shutdown/reboot controls as part of this plan.

Do not silently exclude other inconvenient features. Tray behavior, Open with,
file associations, desktop notifications, updates and similar platform-dependent
actions must be audited individually. Propose a meaningful browser/server
equivalent, or ask the owner to approve an exception before counting the feature
as resolved. Native-window appearance is not itself functional parity.

Parity includes options, validation, confirmations, failures and recovery—not just
a button with the same name. Sharing the same engine does not establish parity.

### Baseline rules

- Inventory the **current fork's desktop implementation**, not screenshots alone.
- Record the exact revision/worktree snapshot used for implementation acceptance.
- The later official-app integration is a separate task in
  [upstream-merge-plan.md](upstream-merge-plan.md). Refresh the parity inventory after
  any merge; do not merge upstream merely to execute this plan.
- An engine limitation is not automatically a Web gap. For example, do not promise
  tracker removal or arbitrary file priorities unless the desktop baseline supports
  them. Record unsupported-both capabilities separately as enhancements.

## 2. Build the authoritative parity ledger first

Create `docs/web-functional-parity-matrix.md` during implementation. This plan's
summary is a starting point, **not an exhaustive audited matrix**.

Inventory:

- Every `Cmd` in `crates/app/src/app.rs`, every menu/context-menu action and shortcut.
- `Settings` fields, option tabs and per-download overrides.
- Desktop dialogs, especially confirmation, password and failure/retry prompts.
- Torrent, RSS, media/gallery, extraction and account-management workflows.
- Import/export formats, platform integrations and helper-management operations.
- Existing Web UI, CLI and extension capabilities so no working behavior is lost.

Each ledger row must have:

| Field | Required content |
| --- | --- |
| Desktop capability | Specific action/option and its source symbol/path |
| Preconditions | Transfer state, source kind, helper/account and platform requirements |
| Web equivalent | Entry point, dialog/tab and API operation |
| Authorization | Owner, authenticated/bypass user or extension scope |
| Data and side effects | Paths, secrets, overwrite behavior and persistence |
| State contract | Loading, pending decision, success, failure, cancellation and restart |
| Status | Unverified, missing, partial, implemented, verified or approved exception |
| Evidence | Test name and relevant rendered/manual evidence |
| Open limitation | What remains, rather than an implicit checkmark |

- [ ] Enumerate every source action and settings field.
- [ ] Separate real desktop gaps from Web-only improvements.
- [ ] Resolve platform-dependent equivalents and obtain approval for any new exceptions.
- [ ] Turn each missing/partial ledger row into an implementation/test task.

## 3. Initial capability inventory

“Available” below means implemented with existing tests for important paths, not a
claim that the complete desktop feature family has already been verified.

| Area | Current Web position | Work needed for parity |
| --- | --- | --- |
| Add URL/magnet/torrent | Available; Web/Torrent tabs and asynchronous admission results | Full desktop add options and pre-start inspection/decisions |
| Start/pause/remove/delete completed | Available for core paths | Audit Stop all, hidden/filtered selections and every state transition |
| Duplicate links and existing torrent files | Confirmation/reuse and existing-entry/numbered-copy choices available | Audit all remaining desktop duplicate/restart/overwrite choices |
| Progress/details | Status, connections, activity, speed/upload limits and properties available | Match all supported desktop data, tabs and actions; no fabricated values |
| Plain-file tools | Move/rename, source replacement and checksums available | Audit restart-from-zero, batch/multi-file behavior and other source types |
| Torrent controls | Post-add selection, ordering, limits, peers, trackers and recheck available | Pre-start file selection; supported piece/general details and remaining desktop actions |
| Queues/scheduler | Membership, order, concurrency, schedules and start/stop available | Compare every queue/scheduler option and persisted execution behavior |
| Categories | Filtering and custom-category management available | Full built-in category editing, restoration and related desktop choices |
| Batch/link grabber | Missing dedicated flows | Implement supported batch generation, extraction/filtering and selection |
| RSS | Missing Web management | Feeds, rules, matching/previews, jobs and execution history |
| Video/playlists/gallery | Incomplete dedicated controls | Full supported inspection, selection, options, progress and error workflows |
| Site/debrid accounts | Missing Web account management | Secure account/credential workflows and service status/quotas |
| Archive extraction | Settings subset; no complete interactive flow | Manual extraction, password prompts and supported destination/collision choices |
| Import/export | Missing dedicated flows | Desktop-supported text/EF2 formats, previews and safe secret handling |
| Application settings | Validated allowlist, not every option | Audit every server-relevant desktop setting and dependency |
| List customization | Source/status/category/search, sorting, selection and themes available | Columns, ordering, layout persistence, shortcuts and remaining desktop preferences |
| Help/diagnostics/platform actions | Partial or unaudited | Appropriate browser/server equivalents; explicit decisions for native integrations |

## 4. Implementation phases

### Phase A — Shared operations and decision lifecycle

- [ ] Use the existing application loop and engine operations; avoid a second backend
      or a separately implemented downloader.
- [ ] Add typed, versioned request/result contracts and capability reporting for each
      new operation. Do not deserialize arbitrary IPC commands or raw settings.
- [ ] Standardize pending decisions: preview, explicit confirmation, cancellation,
      expiry, changed-state validation and idempotent confirmation.
- [ ] Preserve default no-overwrite behavior. Any desktop-equivalent destructive
      overwrite/restart action needs a clearly named, specific confirmation and a
      recheck of the target immediately before the action.
- [ ] Preserve root/path constraints, queued destination and sidecar reservations,
      and pending-move targets. Never create two writers for one destination.
- [ ] Distinguish closing a dialog from cancelling a submitted server operation.
- [ ] Keep ordinary drafts, clear secrets and protect newer edits from late replies.
- [ ] Define how accepted/pending operations are reconciled after a lost response or
      restart. Never present scheduling as completed work.

**Exit:** new features can reuse one authorization/decision/result model without
weakening existing owner, extension, filesystem or credential boundaries.

### Phase B — Complete ordinary download and file workflows

- [ ] Match desktop Add URL/File Info options, including supported request metadata,
      authorization, destination/name, category and start/later choices.
- [ ] Implement Stop all with clear scope, including downloads hidden by filters.
- [ ] Implement Download again/from the beginning and the desktop's remaining
      duplicate choices, with separate confirmation for destructive replacement.
- [ ] Audit Properties, source refresh, limits, connections and progress information.
- [ ] Complete supported multi-selection actions, move/rename and destination changes
      across desktop-supported transfer types. Do not assume a plain-file operation
      is valid for a torrent or aggregate container.
- [ ] Provide appropriate equivalents for Open, Open with and Open containing folder.
      A browser cannot open a server-local path on the user's computer; design a
      controlled file-delivery/browsing workflow where appropriate, with permission,
      traversal, content-disposition and active-content protections.

**Exit:** ordinary downloads can be managed end-to-end without desktop intervention.

### Phase C — Complete torrent admission and management

- [ ] Add pre-start metadata preview and file selection for uploaded torrents,
      torrent URLs and magnets, with explicit loading, timeout and cancellation.
- [ ] Expose supported destination, category, queue, order and start/later options
      before beginning the transfer.
- [ ] Integrate existing-file consent with pre-start selection. Check every path the
      engine may touch, including unselected files sharing pieces and padding rules.
- [ ] Preserve matching data and hash-check it; never equate a filename match with a
      verified piece. Respect queued/paused intent.
- [ ] Audit desktop Files/Peers/Trackers/Pieces/General details, magnet-copy action,
      recheck/reannounce, sharing/seeding limits and torrent-specific settings.
- [ ] Implement only tracker/priority actions that the desktop engine actually offers.

**Exit:** every supported desktop torrent action and admission decision has a tested
Web equivalent, including existing-data reuse and stopped/completed torrents.

### Phase D — Queues, scheduler, categories, batch and import/export

- [ ] Compare every scheduler control and stop/retry behavior against the desktop;
      test server timezone, overnight schedules, one-shot dates and restart behavior.
- [ ] Complete queue membership/reordering/context actions and built-in queue rules.
- [ ] Support all desktop-editable built-in category properties and restoration,
      preserving stable references and existing files.
- [ ] Implement batch URL/range workflows and supported link-grabber functionality,
      with preview, validation, selection and bounded request sizes.
- [ ] Implement desktop-supported import/export formats, including text and EF2 where
      applicable. Preview records before adding; define duplicates, destinations and
      credentials explicitly. Never export secrets implicitly.

**Exit:** desktop list organization and bulk intake need no local GUI.

### Phase E — RSS, media, gallery and helper workflows

- [ ] RSS feed CRUD, refresh/status, rule CRUD, match preview and rule destinations.
- [ ] Persist rule history/deduplication and make failed feed/metadata jobs visible.
- [ ] Media page inspection and supported format/audio/subtitle selections.
- [ ] Playlist/gallery inspection, item selection, naming/destinations and job controls.
- [ ] Show helper availability/version and actionable errors. Expose supported helper
      installation/update behavior only through a deliberate server-safe mechanism.
- [ ] Test cancellation, partial completion, retries, unavailable/private sources and
      a server restart while helpers or discovery jobs are running.

**Exit:** all desktop-supported content-source workflows can be completed headlessly.

### Phase F — Accounts, secrets and extraction

- [ ] Site-login and debrid account management, connection tests, service errors and
      desktop-supported quota/status information.
- [ ] Deliberate server-side secret storage, redacted reads, replacement/deletion and
      password-confirmed sensitive operations as appropriate. Owner secrets must
      never appear in browser storage, logs, exported diagnostics or device replies.
- [ ] Manual and automatic extraction workflows, passwords, collisions, destinations,
      cancellation and failure recovery, following desktop engine capabilities.
- [ ] Treat archive paths and links as untrusted; enforce extraction-root and resource
      limits before enabling broader file operations.

**Exit:** credentials and password-protected content do not require a desktop prompt.

### Phase G — Settings, workspace and platform equivalents

- [ ] Audit all desktop settings: networking/proxy/DNS/adapter, connection/retry limits,
      global/per-transfer limits, capture rules, storage, seeding and source helpers.
- [ ] Distinguish global settings from current-transfer overrides; document options
      taking effect only after restart and enforce dependencies/ranges server-side.
- [ ] Complete list columns, sorting/filtering, queue/category navigation, shortcuts,
      language/RTL, themes and persistent workspace preferences where supported.
- [ ] Map notifications, clipboard, file/protocol associations, help, shortcuts,
      diagnostics, support links and update workflows to the browser/server model.
- [ ] Review every non-shutdown completion action separately. Do not turn native
      "run a program" options into an unrestricted remote-command API.
- [ ] Resolve lifecycle/update behavior for native servers versus immutable containers.
      Do not call an unimplemented desktop option "not applicable" without approval.

**Exit:** every settings/platform row is verified or has an owner-approved exception.

## 5. Security and compatibility gates

Every phase must preserve:

- Owner authentication, protected cookies, CSRF and password-confirmed security changes.
- Explicitly scoped extension devices: capture/control never implies owner administration.
- Existing-file consent, non-overwriting moves and distinct queued destinations.
- Root/path, archive, uploaded-file and request-size validation.
- Deliberate outbound-request policy, including redirects, DNS and metadata/helper
  endpoints. The current private-network-source limitation remains a security gap.
- Redaction of credentials, cookies, pairing codes and private operational state.
- Official desktop/local-extension compatibility and minimal hooks in upstream-owned
  files; put fork-specific handlers/tests in separate modules where practical.

Features needing wider filesystem, process or network access require a security
review before exposure, not merely a copied desktop button.

## 6. Verification and definition of done

For each ledger row:

- [ ] Compare the real desktop behavior with the Web operation.
- [ ] Cover normal, empty, busy, invalid, denied, cancelled and failed states.
- [ ] Test supported transfer states/types and concurrent changes where material.
- [ ] Test unauthorized, extension-token and missing-CSRF rejection for owner actions.
- [ ] Verify disk/database side effects, not only responses, IDs or notifications.
- [ ] Verify persisted behavior across restart where the desktop persists it.
- [ ] Add browser interaction coverage and real-headless coverage where applicable.
- [ ] Review desktop/mobile, long names, both themes, keyboard access and RTL for
      changed surfaces; do not remove a function to make a narrow layout fit.

Run affected Rust tests, Clippy, formatting, English catalog checks, Chromium fixtures,
real-headless integration and extension regressions. Complete actual Firefox remote
testing and the deployment acceptance work before making platform-specific claims.

Linux/Docker/Compose/systemd runtime acceptance remains deferred until application
features are finished, per the owner's existing instruction. Use
[linux-server-testing.md](linux-server-testing.md) afterward. Feature parity on the
development host is not proof of deployment parity on an untested platform.

### Gate for a 100% claim

- Every inventoried desktop action and option has a ledger row.
- Every in-scope row is **verified**, not merely implemented or partially supported.
- Shutdown-after-finish is excluded as agreed; any other exceptions are explicitly
  owner-approved, listed and disclosed in the claim.
- No unresolved decision dialog forces use of a desktop session.
- Evidence exists for the declared desktop revision and supported deployment platforms.
- No critical regression, data-loss or authorization defect is open.

Do not estimate a parity percentage from test counts or feature-family counts.
Passing the current suites is a regression baseline, not the 100% gate.

## 7. Web/server improvements tracked separately

Durable capture/idempotency, backup/restore administration, audit events, public
typed HTTP documentation, large-history virtualization and SSE may be valuable
release requirements. They are **not automatically desktop parity gaps** unless
the inventory establishes a desktop equivalent or equivalent reliability requires
them. Likewise, Web-only authentication/device administration must remain intact.

## Related documents

- [web-development.md](web-development.md) — architecture, validation and remaining work.
- [web-download-tools.md](web-download-tools.md) — current operations and API limitations.
- [web-confirmation-parity.md](web-confirmation-parity.md) — focused admission-prompt audit.
- [ui-redesign-plan.md](ui-redesign-plan.md) — approved desktop-style workspace design.
- [upstream-merge-plan.md](upstream-merge-plan.md) — independent upstream integration task.

Creating this plan does not implement, merge, commit or publish its remaining tasks.
