# Nesting settings: migration audit

Compared against the original configuration form, explanations, and genetic search
at commit `74e739f` (`frontend/index.html`, `frontend/deepnest.js`). The original
form exposed 16 controls plus Restore defaults. All 16 now have native controls
and connected behavior. The deleted web frontend remains available in Git history.

Open **Nesting settings** from the toolbar, project menu, or **Cmd/Ctrl + ,**.
The five sections keep the canvas uncluttered. Each control has an illustrated
explanation with examples, tradeoffs, and a suggested starting point.

| Original control | Before | After | Why |
| --- | --- | --- | --- |
| Display units | Fixed mm | Millimeters/inches across fields, dimensions, and rulers | Change presentation without resizing geometry |
| Part spacing | Gap field | Gap field and illustrated clearance guide | Explain clearance and its interaction with shared cuts |
| Curve tolerance | Fixed 0.1 mm | Adjustable SVG/DXF flattening tolerance | Trade approximation error against computation |
| Rotations | 1, 2, 4, 8, or 16 only | Every integer from 1–32 | Preserve the original orientation choices |
| Placement type | Bounding box only | Gravity, Bounding box, Compact | Optimize for a usable offcut, rectangle, or convex area |
| Rough approximation | Omitted | Conservative convex envelopes for nesting | Speed up complex geometry while exporting original cut outlines |
| CPU cores | One search worker | 1–8 parallel candidate evaluations with a shared NFP cache | Restore control over resource usage |
| SVG scale | Fixed 96 units/inch | Configurable fallback; physical dimensions take priority | Handle unitless source files from different editors |
| Endpoint tolerance | Fixed DXF stitching; no SVG stitching | Configurable, unambiguous endpoint joining in both formats | Repair tiny gaps without inventing large closing edges |
| DXF import units | File units only | Auto, points, picas, inches, mm, cm | Override missing or incorrect file metadata |
| DXF export units | Fixed mm | Points, picas, inches, mm, cm | Match downstream coordinate conventions |
| Merge shared lines | Disabled | Union touching and partially overlapping cut segments in SVG/DXF | Avoid duplicate laser passes; requires zero gap |
| Material/time ratio | Fixed material-only scoring | Cutting-time weight from 0–1 | Prefer longer shared edges when enabled |
| Population size | Single best-candidate mutation loop | Configurable population with selection, crossover, and elitism | Keep multiple promising arrangements in the search |
| Mutation rate | Hardcoded mutation schedule | Configurable 1–50% order/rotation mutation | Control how widely the search explores |
| Restore defaults | Omitted | Reset a draft, then Apply or Cancel | Restore configuration without changing stock dimensions or parts |

## Behavior and compatibility

- Settings are saved in `.deepnest` projects. Earlier Iced project files load with
  defaults for missing settings. Apply validates the complete draft; Cancel makes
  no project changes, and Undo restores applied edits.
- The search can use a fixed evaluation budget or continue until Stop. Stop keeps
  the best completed layout and cancels all candidate workers.
- Geometry remains in millimeters internally. Display-unit changes do not alter
  imported size, invalidate a valid layout, or change file conversion settings.
- Curve tolerance, endpoint tolerance, SVG scale, and DXF import units affect the
  next import. Reimport existing source files to rebuild their polygon outlines.
- Rough approximation uses a containing hull only for placement. Exact shared-edge
  scoring is skipped in this mode; export still merges actual touching cut edges.
- Shared-line export produces an open cut network. Disable it when downstream
  software requires each part as an independent closed contour.
- DXF has no standard point/pica unit code. Those export modes use unitless
  coordinates; select the corresponding import override on a round trip.
- The old remote conversion-server setting was internal infrastructure, not one
  of the 16 form controls. SVG and DXF conversion now runs locally. This audit is
  about nesting settings; it does not claim support for the original cloud/CDR
  services or every legacy file format.

Defaults intentionally suit the native workflow: mm display, 96 units/inch SVG,
automatic DXF input units, mm DXF output, 0.1 mm curve tolerance, 0.01 mm endpoint
tolerance, two CPU workers, population 10, and mutation 10%. All are editable.

## Verification

Regression coverage includes physical-unit invariance, settings validation and
Apply/Cancel/Undo, project round trips, configurable SVG tolerances/scale, DXF
unit conversions, matching SVG/DXF shared cuts, population preservation,
parallel search budgets, continuous-search cancellation, conservative rough
envelopes, and actual placement/laser scoring. A native-engine regression also
checks all allowed rotations are tried when a part initially does not fit.
