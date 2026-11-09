# MDM web workspace

Development-only design reference; this file is not served by the embedded asset router.
The root DESIGN.md remains the marketing website's system.

## Direction contract

**THESIS:** A downloads-first operating workspace, explicitly selected by the owner: transfer
rows lead, actionable errors stay beside files, administration is a separate task.

**OWN-WORLD:** A deep blue navigation rail, neutral work surfaces, restrained blue selection
and controls, green completion, red errors; system UI typography with tabular transfer figures.
Light follows daytime desktop use; dark supports server administration at night. System mode
follows the browser, with a persistent local override.

**STORY:** Authenticate, see running/failed transfers, add or capture a download, inspect it,
then manage queues, schedules, categories, settings and extension devices without a desktop.

**FIRST VIEWPORT:** Compact product header, left status/navigation rail, search and category
filters directly over the download table, primary add action at the top end. Details appear
beside the list on wide screens and below it on narrow ones. Queue actions remain nearby.

**FORM:** User-pinned downloads workspace and qBittorrent-style authentication controls;
functional web controls, not a desktop dialog reproduction. No visual-concept lottery is used
to override that approved structure. The signature is the transfer's progress and connection
breakdown in place, not a decorative monitoring dashboard.

**FINISH:** unreviewed and undocumented is unfinished; this build ends with the finish review,
the verdict, DESIGN.md, and every shipping raster carrying its provenance.

## Component rules

- Header 76px; desktop rail 230px, compact rail 195px; narrow navigation becomes a horizontal strip.
- Work surfaces 10px corners; controls 6–7px; dividers separate rows rather than nested cards.
- Keyboard focus is a 3px blue outline, not color change alone. Native forms, semantic table,
  real buttons, dialogs only for destructive confirmation and credential confirmation.
- All directions use logical properties; long filenames truncate in lists but wrap in details.
- Unknown sizes and offline snapshots stay explicitly unknown/stale. No fabricated charts,
  transfer statistics, user counts or claimed helper capabilities.
- Application strings use the existing catalog endpoint. New untranslated entries fall back to
  English; existing translations and RTL remain available.
- The existing MDM logo is used unmodified. No new raster artwork is introduced.
