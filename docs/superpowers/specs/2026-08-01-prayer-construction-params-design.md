# Prayer and Construction params audit

Date: 2026-08-01

## Problem

`lib/Database.ini` backs the `+params <skill> <query>` command. The combat and
Slayer sections are generated from the wiki by `reinze-lib-runescape/bin/cmb-xp.py`
and are out of scope. Every other section is hand-maintained and has drifted:
values that predate Old School, RS3-only content, truncated decimals, and keys
the lookup can never match.

Prayer (59 entries) and Construction (389 entries) are the next two sections in
the ongoing audit. Woodcutting, Thieving, Cooking, Fletching, Firemaking,
Runecraft, Mining, Fishing, Agility, Hunter and Herblore are already done.

### How lookup works

`reinze-lib-runescape/src/params.rs` loads the INI, selects the section by
capitalised skill name, and filters keys with a **case-insensitive substring
match**. The user's query has spaces replaced by underscores before matching;
results have underscores replaced by spaces for display, and are capped at 10.

Three consequences drive this work:

- A key containing a literal space can never match a multi-word query, because
  the query's spaces became underscores.
- A key using `-` where a user would type a space has the same problem.
- An empty value renders as a bare `xp` with no number.

### Faults already confirmed

Construction:

- `Asgarnian_ale184=`, `Mahogany_Bookcase420=`, `Bench_with_Vice140 =` — the
  value is glued onto the key and the value is empty.
- `Oak_Shaving Stand`, `Bench_with_Vice140 ` — literal spaces, unmatchable.
- 26 keys contain `-` where a space is natural: `Brown-rug`, `Marble_cape-rack`,
  `Tool_store-1`, `Teak_Shelves-1`, `Oak-prize_chest` and others.
- `Crafting_Table-2=1` and `Crafting_Table-3=2` are not experience values.

Prayer has no mechanical faults. It has content gaps and at least one duplicate
(`Baby_Dragon_Bones` and `Babydragon_Bones` are the same item).

## Data source

The wiki exposes per-item structured data through two templates. Both are the
item's own page rather than a skill-page summary table, which is the sourcing
rule this audit follows — summary tables were found to disagree with item pages
roughly one value in ten.

`{{Infobox Construction}}` — 500+ mainspace transclusions, fields `name`,
`level`, `experience`, `room`, `hotspot`, `flatpack`.

`{{Prayer info}}` — 193 mainspace transclusions, fields `name`, `level`, `xp`,
`type`.

Both enumerate exhaustively through the MediaWiki API:

```
action=query&list=embeddedin&eititle=Template:Infobox%20Construction&einamespace=0&eilimit=500
```

Page wikitext comes from `?action=raw`.

Neither Cargo (`action=cargoquery`) nor Semantic MediaWiki (`action=ask`) is
installed on the wiki, and no consolidated furniture list page exists, so
per-page fetching is the only structured route. The room pages (`Parlour`,
`Kitchen`, …) carry the same data in wikitables and serve as a cross-check.

## Approach

A throwaway script generates a proposal; every keep, drop and rename decision is
made by hand from its report. Neither pure hand-transcription (389 entries is too
many to do reliably) nor blind script output (the INI holds legitimate entries
the templates do not cover) is sufficient alone.

### Stage 1 — Fetch

Enumerate transclusions for both templates, following `continue`. Fetch each
page's raw wikitext. Cache to disk so re-runs cost nothing, rate-limit politely,
and send a descriptive User-Agent. Roughly 700 requests, once.

### Stage 2 — Parse

Extract the template block from each page and read its fields.

- `|experience = 2,230` — strip thousands separators.
- Versioned pages carry `experience1`, `experience2`, … alongside `version1`,
  `version2`, …. When every version shares one experience value, emit a single
  entry under the base `name`. When versions differ, emit one entry per distinct
  value, keyed `Name_(Version)`. `Gilded altar` is the common case: four
  versions, one value, one entry.
- A missing or unparseable field is reported, never silently dropped.

### Stage 3 — Reconcile

Join the parsed data against the current INI section into four buckets:

| Bucket | Meaning |
| --- | --- |
| unchanged | key and value both agree |
| changed | key matches, value differs |
| wiki-only | candidate addition |
| INI-only | candidate drop or rename |

### Stage 4 — Review and emit

The INI-only bucket carries the judgment. Entries such as `Isafdar=464`,
`Lumbridge=314`, `Miscellanians=311` (Portal Nexus destinations) and
`Dagannoth=2738`, `Hellhound=2236`, `Steel_dragon=3162` (Oubliette and dungeon
guards) are not furniture infoboxes and will land here. Each is resolved against
its own wiki page individually — corrected, renamed, or dropped as an RS3-ism.
None are removed merely because the script did not find them.

A sample of the changed bucket is spot-checked against the rendered page before
the section is written.

Keys are then normalised to the wiki's exact item name with every separator an
underscore, and the section is written out.

Construction is emitted as one alphabetically sorted block, which is what it
approximates today. Prayer keeps its existing two-block layout — bones and ashes
first, a blank line, then ensouled heads — each block sorted within itself. The
blank line is already there and the `ini` crate reads it fine.

## Scope decisions

**Construction: full rebuild.** All 389 values are re-derived, not just the
broken ones. This is what the Herblore and Woodcutting commits did, and it is
the only way to catch values that are wrong but well-formed.

**Keys: normalised to wiki names with underscores.** This is the only change
that makes the 26 hyphenated keys reachable. It touches many lines.

**Prayer: keep the altar variants, do not expand them.** Only dragon, lava
dragon, superior dragon and wyvern bones have `(Gilded_Altar)` and
`(Ectofunctus)` rows today. Those four are kept and their multipliers verified
(Gilded altar ×3.5, Ectofuntus ×4). Adding the pair for all ~25 bone types would
triple the section and push common queries such as `bones` past the 10-result
display cap.

The `(Ectofunctus)` rows are respelled `(Ectofuntus)`. That is the in-game and
wiki spelling — `Ectofunctus` is only a redirect — so a user typing the correct
name currently matches nothing. This is the same class of fault as the
hyphenated Construction keys, so it is fixed for consistency even though the
decision to keep the variants was about expansion rather than naming.

Prayer gaps to close: wyrm, drake and hydra bones; the ashes line; and the
`Baby_Dragon_Bones` / `Babydragon_Bones` duplicate.

## Out of scope

- The combat sections (`Attack`, `Strength`, `Defence`, `Ranged`, `Hitpoints`)
  and `Slayer`. Script-maintained; a consolidation with
  `reinze-lib-runescape/src/npc/data.rs` is pending.
- The 10-result display cap in `params.rs:50`. A fuller Construction section
  makes truncation on broad queries such as `oak` more likely. This is
  pre-existing lookup behaviour and changing it is a code change, not a data
  change. Flagged, not fixed.
- Any other skill section.

## The script is not committed

It lives in the job's temp directory. Every prior data commit in this series
touched `lib/Database.ini` alone, and a consolidation of the existing
script-generated combat data is already pending — a second committed generator
would cut across it.

## Verification

- Re-run the mechanical fault scan over both sections: no empty values, no
  literal spaces in keys, no non-numeric values, no duplicate keys under
  case-insensitive comparison.
- `cargo test -p reinze-lib-runescape` passes.
- The INI still parses: load it through the same `ini` crate the bot uses and
  confirm both sections read back with the expected entry counts.
- Spot-check a sample of changed values against their rendered wiki pages.

## Deliverable

Two commits following the established pattern, Prayer first:

```
chore(data): Update Prayer params from wiki
chore(data): Update Construction params from wiki
```

Each body states what changed and why, including what was deliberately left
alone.
