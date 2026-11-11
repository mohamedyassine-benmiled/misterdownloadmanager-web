# Desktop-style web UI redesign

Status: **approved plan; implementation not started.**

Approved on 2026-11-11. This document records the agreed redesign of the
self-hosted web workspace, not the marketing website.

## Goal and visual reference

Make the web workspace feel like **Mister Download Manager desktop running in a
browser**: a persistent command toolbar, folder-tree navigation, a compact download
table, and desktop-style modal dialogs. Preserve every existing web capability.

Mode: **Operate**. Prioritize recognizable controls, fast scanning, clear transfer
states and predictable interaction.

The user confirmed MDM desktop as the reference, and requested **both a Source
column and All / Web / Torrents filter tabs**.

Reference screenshots:

- `website/assets/screens/main-light.webp` — toolbar, tree and table.
- `website/assets/screens/main-dark.webp` — dark appearance.
- `website/assets/screens/progress-light.webp` — web transfer dialog.
- `docs/images/progress-torrent.webp` — torrent progress and tabs.
- `docs/images/add-url.webp` — compact Add URL dialog.

These screenshots establish structure and visual character. Check current desktop
code when implementing behavior; screenshots may show an older application version.

## Main workspace

```text
Product header                                  Theme / Account
Persistent toolbar: Add / Torrent / Start / Pause / Remove / Settings / Scheduler
-------------------------------------------------------------------------------
Folder sidebar       | All / Web / Torrents                         Search
                     | Source | Filename | Progress | Size | Speed | Remaining
                     | Download rows
-------------------------------------------------------------------------------
Download count / Running count / Total transfer speed
```

- Keep the toolbar visible while the download list scrolls.
- Use compact desktop-style controls, restrained borders, consistent icons and
  readable transfer figures; retain light, dark and system themes.
- Keep selection-based actions visible but disabled when unavailable.
- Clearly distinguish selected-download actions from whole-queue actions.
- Keep server connection/offline state visible. Unknown values stay unknown.
- Use server metadata to distinguish Web and Torrent; do not infer source from a
  filename extension. Verify the list API exposes the required metadata.
- Combine source tabs, sidebar filters and search consistently.

## Folder-style sidebar

```text
All downloads
  [Configured categories, with suitable folder/file-type icons]
Unfinished
  [Useful status subfilters]
Finished
Queues
  [Configured queues]

Manage
  Categories
  Queues and scheduler
  Application settings
  Authentication
  Extensions and devices
```

- Use folder icons, disclosure arrows, clear selected states and counts.
- Read categories and queues from server configuration rather than hardcoding
  example names. Familiar categories can include Compressed, Documents, Music,
  Programs, Video and Torrents when configured.
- Unfinished includes downloading, queued, paused/stopped and failed downloads.
- Define category, status and queue filter combinations explicitly during
  implementation; counts must match their documented scope.
- Preserve expansion state while live data refreshes.

## Download progress modal

A single click on a download row opens its details modal. Checkboxes and inline
action buttons perform their own actions without opening the modal. Provide an
equivalent keyboard action.

Follow the desktop dialog's composition:

- Filename and close button in the title bar.
- Status, source address, size, downloaded amount, speed and remaining time.
- Full-width progress bar.
- Play/resume and pause controls with state-aware availability.
- Expandable connection details.

Organize all existing web controls into tabs:

| Tab or section | Required contents |
| --- | --- |
| Download status | Live statistics, connections and activity |
| Speed limiter | Download limit and torrent upload limit |
| Properties | Destination, category and description |
| File tools | Move/rename, replace expired link and checksum verification |
| Torrent tabs | Files, peers, trackers and download-order controls |

Retain torrent recheck and reannounce actions. Show file selection restrictions,
unavailable metadata, busy operations and errors explicitly. Preserve existing
owner-only control boundaries.

Desktop-only details, piece/connection visualizations and completion actions must
be checked against available API data before implementation. Do not display
fabricated statistics or controls that cannot work.

## Add-download modal

Replace the inline add panel with a compact desktop-inspired dialog containing:

- URL or magnet address.
- Torrent-file picker and selected-file feedback.
- Category selection.
- Start now, add paused or queue selection.
- Clear Add and Cancel actions, validation and submission feedback.

The toolbar's Add Torrent shortcut opens the same dialog in torrent mode.
Preserve the existing URL/torrent mutual-exclusion behavior and file validation.

The desktop Add URL dialog also has authorization fields. Check server support
before adding these; they are a capability investigation, not an already-supported
feature promised by this redesign.

## Management modals

Open the following over the download list instead of replacing it with a page:

- Queues and scheduler, including queue membership and scheduling fields.
- Categories, including extensions and destination folders.
- Application settings.
- Web UI authentication, credentials and login bans.
- Browser extensions and devices, pairing and revocation.

Use internal tabs or navigation for larger dialogs. Keep the title and close
control visible, and save actions readily accessible while content scrolls.
Return to the same list position, filters and selection when dismissed.

## Shared modal contract

- Clicking outside a dialog closes it immediately, as requested.
- Escape and the close button also dismiss it.
- Backdrop dismissal must not be triggered by clicking within the dialog or by a
  drag that begins inside it and ends outside.
- Closing does not pause a download, submit a form, confirm deletion or revoke a
  device. Dismissing a confirmation means cancel.
- Preserve ordinary unsaved edits as in-memory drafts when reopening the same
  dialog/item. Offer an explicit way to discard a draft.
- Clear password fields and pairing secrets on dismissal; never persist them as
  ordinary drafts. Clear all drafts on sign-out or session expiry.
- Closing during a submitted request does not imply server-side cancellation.
  Report the outcome without unexpectedly reopening the dialog.
- Trap focus within the active modal and make background controls inert. Restore
  focus to the opener, or a sensible fallback if its download was removed.
- A confirmation opened from a management dialog must have explicit topmost
  dismissal/focus behavior; closing it must not dismiss its parent accidentally.
- Preserve polling and current draft-race protections: delayed responses must not
  overwrite newer edits, reopen dismissed dialogs or populate a different item.

## Icons and accessibility

- Use one consistent SVG icon family or reusable local icon set, not emoji.
- Prefer familiar icons for start/play, pause, remove, add, settings, scheduler,
  folders and close.
- Icon-only buttons need translated accessible names and hover/focus tooltips.
- Keep visible text for ambiguous actions such as Replace address and Reannounce.
- Preserve keyboard navigation, visible focus, semantic tables and labeled tabs.
- Do not communicate source, status or errors by color alone.
- Keep long filenames, URLs, translations and right-to-left layouts usable.

## Responsive behavior

- Desktop: compact table, persistent toolbar and folder sidebar; centered dialogs.
- Narrow screens: collapsible folder navigation and a compact persistent toolbar
  with clearly accessible overflow actions.
- Phone dialogs become near-full-screen with reachable close/save controls and
  internally scrolling content. Backdrop dismissal applies wherever backdrop is
  exposed; close and Escape remain available.
- Keep source identification visible in compact rows and retain access to all
  secondary data. Avoid page-wide horizontal overflow.

## Implementation sequence

- [ ] Inventory existing controls and API fields; map every control to its new home.
- [ ] Build reusable modal lifecycle, draft handling and icon-button primitives.
- [ ] Implement persistent toolbar, folder tree, source column and source tabs.
- [ ] Move download details into desktop-style tabbed progress dialogs.
- [ ] Convert Add Download and Add Torrent to the shared add dialog.
- [ ] Convert all management sections to dialogs.
- [ ] Verify behavior, feature parity, accessibility, responsiveness and themes.
- [ ] Update `crates/remote/web/DESIGN.md` to describe the implemented design.

Primary implementation files are `crates/remote/web/index.html`, `app.css` and
`app.js`. Keep reusable UI logic separated where practical. If new asset files are
introduced, update the embedded asset router and fixture server accordingly.

The existing web DESIGN.md describes inline details and page-based management;
this approved plan supersedes those choices for the redesign. The root product
context describes the marketing website and is not this workspace's UI brief.

Upstream integration remains a separate task in
[upstream-merge-plan.md](upstream-merge-plan.md).

## Acceptance checklist

- [ ] A downloading row opens a recognizable MDM-style live progress modal.
- [ ] Checkbox selection and inline actions never accidentally open details.
- [ ] Add Download and Add Torrent open dialogs with all current add options.
- [ ] Every management entry opens a modal over the preserved download list.
- [ ] Outside click, Escape and close work consistently and never submit actions.
- [ ] Ordinary drafts survive dismissal; secrets are cleared.
- [ ] Polling and late save responses cannot erase newer edits or reopen dialogs.
- [ ] Source column and All / Web / Torrents tabs correctly classify both uploaded
      torrents and magnet downloads, independently of filenames.
- [ ] Folder navigation, counts, source filters, search and selection work together.
- [ ] Toolbar remains visible during list scrolling; action scope is unambiguous.
- [ ] All current file, torrent, queue, category, settings and security controls
      remain reachable with their existing permissions and safeguards.
- [ ] Loading, empty, failed, offline, expired-session and unavailable-metadata
      states remain understandable.
- [ ] Keyboard-only use, focus restoration, mobile layouts, RTL, long names and
      both themes pass a rendered review.
- [ ] Update and run `tools/web/test.mjs` for changed interactions and
      `tools/web/headless.mjs` for affected real-server flows; verify embedded
      asset delivery if the asset structure changes.

Implementation and verification results should be recorded separately from these
planned acceptance criteria.
