---
name: MDM web workspace
description: Desktop-familiar download management in the browser
colors:
  workspace-bg: "#f3f5f8"
  surface: "#ffffff"
  text: "#172338"
  accent: "#155cc4"
  navigation: "#112b4f"
  dialog-bg: "#f0f0f0"
  dialog-sheet: "#f8f8f8"
  dialog-text: "#171717"
  dialog-line: "#c8c8c8"
  transfer-progress: "#098609"
typography:
  body:
    fontFamily: "Segoe UI, system-ui, -apple-system, sans-serif"
    fontSize: "14px"
    lineHeight: 1.5
  progress-dialog:
    fontFamily: "Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
    lineHeight: 1.35
rounded:
  surface: "10px"
  progress-dialog: "3px"
  desktop-control: "2px"
spacing:
  small: "8px"
  medium: "16px"
---

# MDM web workspace

Development-only design reference; this file is not served by the embedded asset router.
The root DESIGN.md remains the marketing website's system.

## Direction contract

**THESIS:** A downloads-first operating workspace with the familiar MDM desktop
progress window. Transfer rows lead; administration stays in sidebar-opened dialogs.

**OWN-WORLD:** A deep blue navigation rail, neutral work surfaces, restrained blue selection
and controls, green completion, red errors; system UI typography with tabular transfer figures.
Light follows daytime desktop use; dark supports server administration at night. System mode
follows the browser, with a persistent local override.

**STORY:** Authenticate, see running/failed transfers, add or capture a download, inspect it,
then manage queues, schedules, categories, settings and extension devices without a desktop.

**FIRST VIEWPORT:** Product header, persistent command toolbar, left folder/status
tree, source tabs and search above the table. The toolbar holds Add, selected-item
actions, Delete completed and a queue selector with Start/Stop queue. Settings and
scheduler appear only in the sidebar. Details open as a centered desktop-style modal.

**FORM:** User-pinned MDM desktop reference, corrected after local review. The progress
window reproduces its compact title bar, gray status sheet, native-looking tabs,
green meter and connection table. Web-only tools remain reachable in extra tabs;
unavailable native capabilities are not represented by pretend controls.

**FINISH:** unreviewed and undocumented is unfinished; this build ends with the finish review,
the verdict, DESIGN.md, and every shipping raster carrying its provenance.

## Component rules

- Header 76px; desktop rail 230px, compact rail 195px; narrow navigation becomes a horizontal strip.
- Work surfaces 10px corners; controls 6–7px; dividers separate rows rather than nested cards.
- Keyboard focus is a 3px blue outline, not color change alone. Native forms, semantic
  tables and native modal dialogs; Escape/backdrop dismiss without submitting.
- Progress dialog is 546px wide (capped to the viewport), with 12px UI text, 11px
  status/table values, a 32px title bar and 16px green meter. Gray/white native-style
  surfaces become charcoal in dark mode. Status tabs sit above the sheet; Files,
  Peers, Trackers and General sit below the progress controls for torrents.
- Use the original MDM Aero category/status and queue artwork, embedded through an
  allowlisted `desktop-icons/` route. Source/license:
  `crates/app/assets/themes/aero/icons/SOURCES.txt`. Web-only security/device entries
  use matching-scale labeled SVG controls. Artwork is reused without modification.
- Add Download has Web/Torrent tabs; separate address drafts survive switching.
- Accepted adds close the dialog. Torrent admission is confirmed separately in the
  workspace's result notice; pending metadata is not displayed as success. Errors
  explain non-overwriting destination refusal. This notice tracks the current page
  session only, not a durable job history.
- Torrent upload and download limits live in Speed Limiter; Files and General keep
  selection and ordering controls, respectively.
- Existing torrent data requires an owner-only confirmation naming the destination,
  count and preview paths, with unchecked consent to replacing nonmatching data.
  Dismissal never approves reuse. Pending consent retains a Review action; expiry
  becomes a clear, dismissible result. Queued torrents verify on start.
- Duplicate Web links offer existing-entry inspection or a separately named copy;
  do not silently overwrite an existing download because of a desktop preference.
- Delete completed removes history only, after confirmation; never delete files as
  a side effect. Queue actions explicitly scope the selected configured queue.
- All directions use logical properties; long filenames truncate in lists but wrap in details.
- Unknown sizes and offline snapshots stay explicitly unknown/stale. No fabricated charts,
  transfer statistics, user counts or claimed helper capabilities.
- Application strings use the existing catalog endpoint. New untranslated entries fall back to
  English; existing translations and RTL remain available.
- Webmister Download Manager is the owner's fork identity. Its authored mark is
  `brand/webmister.svg`; `tools/web/brand-assets.mjs` generates the Web/extension PNGs.
  Desktop engine names, data paths and original category/toolbar artwork stay intact.

## Reference and verification

The progress-window reference is `website/assets/screens/progress-light.webp`;
torrent layout comes from `docs/images/progress-torrent.webp`. The approved plan and
review corrections are in `docs/ui-redesign-plan.md`. Fixture captures live under
`target/web-shots/`; real-server test state stays isolated under `target/headless-tests/`
and must not be published. Browser/native rendering differences are not a promise
of pixel-identical Windows chrome.
