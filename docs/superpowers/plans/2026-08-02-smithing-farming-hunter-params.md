# Smithing, Farming and Hunter params audit — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-derive the `[Smithing]`, `[Farming]` and `[Hunter]` sections of `lib/Database.ini` from the OSRS wiki's per-item templates.

**Architecture:** Extends the throwaway audit tool built on 2026-08-01, which already has a tested fetch/cache layer and a reconciler. Values become strings end-to-end so Farming's `plant-check-harvest` and Hunter's `xp/wildxp` composites are first-class. Three new parsers feed the existing four-bucket reconcile; a human then works each report and commits one section at a time.

**Tech Stack:** Python 3 standard library only, plus pytest for the tool's tests. Rust/`cargo` for validating the finished INI. No new dependencies anywhere.

## Global Constraints

- **The tool lives at `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/`** and already contains `fetch.py`, `parse.py`, `reconcile.py`, `report.py`, `conftest.py`, their tests, and a warm `cache/`. **37 tests currently pass.** Run the whole suite (`python3 -m pytest -v` in that directory) after every change; no existing test may break.
- **The tool is NEVER committed.** Do not `git add` anything under that path. Only `lib/Database.ini` is committed, by Tasks 5, 6 and 7.
- **`conftest.py` has an autouse fixture** redirecting `fetch.CACHE_DIR` to a per-test `tmp_path`. Leave it alone — it exists because a test once wrote fabricated data into the real cache. No new test may touch the real `cache/` or the network.
- **Repository paths:** the working directory is `/home/rohara/.agent-deck/multi-repo-worktrees/788b004e`, where `rust-reinze` and `reinze-lib-runescape` are symlinks to two **separate git repositories**. `rust-reinze` has NO cargo dependency on `reinze-lib-runescape` (plugins load at runtime as `.so`), so `cargo test -p reinze-lib-runescape` does not work from the bot repo. Run plugin tests with `cd reinze-lib-runescape && cargo test`.
- **Branch:** `data/smithing-farming-hunter-params`, already created, spec committed as `6fb2740`.
- **Do not modify any `.rs` file.** The 10-result cap in `params.rs:50` is known and out of scope.
- **Do not touch** `[Attack]`, `[Strength]`, `[Defence]`, `[Ranged]`, `[Hitpoints]`, `[Slayer]` (script-generated), nor any section other than the three named here. `[Prayer]` and `[Construction]` were audited yesterday and must stay byte-identical.
- **HTTP etiquette:** every request sends `User-Agent: reinze-db-audit/1.0 (claude@ai.ryanohara.co)` and sleeps 100 ms. All responses cache to disk. This is already implemented in `fetch.py`; do not reimplement it.
- **Key format:** the wiki's exact item name with spaces replaced by `_`. Hyphens only where the wiki name genuinely contains one. No literal spaces, no wiki markup (`<`, `[[`, `]]`, `&#`).
- **Commit messages** follow the series: `chore(data): Update <Skill> params from wiki`, body wrapped at 72 columns, ending with `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`. Read `git show 330b941` and `git show 93ea090` and match their voice: plain statements of fact, no bullet lists.

**Existing tool interfaces (already built and tested — use, do not modify):**

```python
fetch.list_transclusions(template: str) -> list[str]   # follows continuation
fetch.fetch_raw(title: str) -> str                     # wikitext, disk-cached
fetch.CACHE_DIR: pathlib.Path

parse.extract_template(wikitext: str, name: str) -> dict[str, str] | None
parse.parse_number(raw: str) -> float | None           # "2,230" -> 2230.0
parse._format_xp(value: float) -> str                  # 2230.0 -> "2230", 4.5 -> "4.5"
parse._key(name: str) -> str                           # "Oak chair" -> "Oak_chair"
parse.UNPARSEABLE: list[str]                           # present-but-unparseable fields

reconcile.read_section(ini_text: str, section: str) -> dict[str, str]
reconcile.format_number(xp: float) -> str
```

---

### Task 1: Reconcile compares strings

`reconcile.reconcile` currently takes `dict[str, float]` and formats each wiki value itself. Farming and Hunter values are composites like `22-1199.5-8.5`, which no float can express. Move formatting out to the callers so reconcile does a plain string comparison.

**Files:**
- Modify: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/reconcile.py`
- Modify: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/report.py`
- Test: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/test_reconcile.py`

**Interfaces:**
- Consumes: nothing new.
- Produces: `reconcile(ini: dict[str, str], wiki: dict[str, str]) -> dict[str, list]` — same five buckets (`unchanged`, `changed`, `wiki_only`, `ini_only`, `collisions`), same shapes, but `wiki` values are now finished INI strings. `format_number` stays exported and unchanged for callers that still hold floats.

- [ ] **Step 1: Write the failing test**

Add to `test_reconcile.py`:

```python
def test_reconcile_compares_string_values():
    # Wiki values arrive pre-formatted; reconcile must not reformat them.
    ini = {"Same": "10", "Differs": "99"}
    wiki = {"Same": "10", "Differs": "40"}

    result = reconcile.reconcile(ini, wiki)

    assert result["unchanged"] == ["Same"]
    assert result["changed"] == [("Differs", "99", "40")]


def test_reconcile_handles_composite_values():
    # Farming packs plant-check-harvest into one value; Hunter uses xp/wildxp.
    ini = {"Apple_tree": "22-1199.5-8", "Baby_impling": "18/20"}
    wiki = {"Apple_tree": "22-1199.5-8.5", "Baby_impling": "18/20"}

    result = reconcile.reconcile(ini, wiki)

    assert result["unchanged"] == ["Baby_impling"]
    assert result["changed"] == [("Apple_tree", "22-1199.5-8", "22-1199.5-8.5")]


def test_reconcile_wiki_only_keeps_the_string_verbatim():
    result = reconcile.reconcile({}, {"Oak_tree": "14-467.3"})
    assert result["wiki_only"] == [("Oak_tree", "14-467.3")]
```

Then update the three EXISTING tests that pass floats — `test_reconcile_sorts_into_four_buckets`, `test_reconcile_matches_keys_case_insensitively`, and any other test calling `reconcile.reconcile` with numeric wiki values — so their `wiki` dicts hold strings instead. For example `{"Same": 10.0, ...}` becomes `{"Same": "10", ...}`, and `{"Dragon_bones": 72.0}` becomes `{"Dragon_bones": "72"}`. Do not change what they assert; only the input type changes.

- [ ] **Step 2: Run tests to verify the new ones fail**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest test_reconcile.py -v`
Expected: the three new tests FAIL. `test_reconcile_compares_string_values` fails inside `format_number` with `TypeError: unsupported operand` or `AttributeError`, because `format_number` is called on a `str`.

- [ ] **Step 3: Make reconcile string-based**

In `reconcile.py`, change the signature and drop the two `format_number` calls. Replace:

```python
def reconcile(ini: dict[str, str], wiki: dict[str, float]) -> dict[str, list]:
```

with:

```python
def reconcile(ini: dict[str, str], wiki: dict[str, str]) -> dict[str, list]:
```

Update its docstring to say wiki values are finished INI strings. Then replace:

```python
        want = format_number(wiki[wiki_by_lower[lower]])
```

with:

```python
        want = wiki[wiki_by_lower[lower]].strip()
```

and replace:

```python
            wiki_only.append((wiki_key, format_number(wiki[wiki_key])))
```

with:

```python
            wiki_only.append((wiki_key, wiki[wiki_key].strip()))
```

Leave `format_number`, `read_section` and `_build_lower_map` exactly as they are.

- [ ] **Step 4: Adapt the existing report.py call sites**

`report.py`'s `gather_construction` and `gather_prayer` still return `dict[str, float]`. Rather than rewrite them, format at the call site. In `main()`, wherever `reconcile.reconcile(...)` is called with one of those dicts, wrap the wiki dict:

```python
{k: reconcile.format_number(v) for k, v in prayer.items()}
```

and likewise for `construction`. Make the same change wherever those dicts are written to the `wiki-*-keys.json` dumps only if the dump currently writes values — if it dumps sorted keys alone, leave it.

The point of this step is only that `report.py` keeps importing and running cleanly after the signature change. Task 4 stops calling the Prayer and Construction paths from `main()` altogether, since both sections are already committed.

- [ ] **Step 5: Run the full suite**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest -v`
Expected: PASS, 40 passed (37 existing with three updated in place, plus the three new).

**No commit.** Throwaway tool.

---

### Task 2: Farming and Hunter parsers

**Files:**
- Modify: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/parse.py`
- Test: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/test_parse.py`

**Interfaces:**
- Consumes: `parse.extract_template`, `parse.parse_number`, `parse._format_xp`, `parse._key`, `parse.UNPARSEABLE` (all existing in the same module).
- Produces:
  - `farming_entries(wikitext: str) -> list[tuple[str, str]]`
  - `hunter_entries(wikitext: str) -> list[tuple[str, str]]`

  Both return `(key, value_string)` pairs, key already underscore-normalised. An empty list means the page carries no relevant template, which is normal.

- [ ] **Step 1: Write the failing test**

Add to `test_parse.py`:

```python
FARMING_FULL = """{{Farming info
|name = Apple tree
|level = 27
|patch = [[Fruit tree patch]]
|plantxp = 22
|checkxp = 1199.5
|harvestxp = 8.5
}}
"""

FARMING_TREE = """{{Farming info
|name = Oak tree
|level = 15
|plantxp = 14
|checkxp = 467.3
}}
"""

FARMING_ALLOTMENT = """{{Farming info
|name = Asgarnian hops
|level = 8
|plantxp = 10.5
|harvestxp = 12
}}
"""

FARMING_NA = """{{Farming info
|name = Belladonna
|level = 63
|plantxp = 92
|checkxp = N/A
|harvestxp = 512
}}
"""

HUNTER_BOTH = """{{Hunter info
|name = Baby impling
|level = 17
|xp = 18
|wildxp = 20
}}
"""

HUNTER_SINGLE = """{{Hunter info
|name = Barb-tailed kebbit
|level = 33
|xp = 168
}}
"""

NO_TEMPLATE_AT_ALL = "Just prose.\n"


def test_farming_joins_all_three_xp_fields():
    assert parse.farming_entries(FARMING_FULL) == [("Apple_tree", "22-1199.5-8.5")]


def test_farming_tree_has_plant_and_check_only():
    assert parse.farming_entries(FARMING_TREE) == [("Oak_tree", "14-467.3")]


def test_farming_allotment_has_plant_and_harvest_only():
    assert parse.farming_entries(FARMING_ALLOTMENT) == [
        ("Asgarnian_hops", "10.5-12")
    ]


def test_farming_omits_unparseable_field_and_records_it():
    # "N/A" must never reach the INI. The field is dropped from the composite
    # and reported so a human can resolve it.
    parse.UNPARSEABLE.clear()
    assert parse.farming_entries(FARMING_NA) == [("Belladonna", "92-512")]
    assert any("Belladonna" in entry and "checkxp" in entry
               for entry in parse.UNPARSEABLE), parse.UNPARSEABLE


def test_farming_returns_empty_without_template():
    assert parse.farming_entries(NO_TEMPLATE_AT_ALL) == []


def test_hunter_joins_xp_and_wildxp():
    assert parse.hunter_entries(HUNTER_BOTH) == [("Baby_impling", "18/20")]


def test_hunter_bare_xp_when_no_wildxp():
    assert parse.hunter_entries(HUNTER_SINGLE) == [("Barb-tailed_kebbit", "168")]


def test_hunter_returns_empty_without_template():
    assert parse.hunter_entries(NO_TEMPLATE_AT_ALL) == []
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest test_parse.py -v`
Expected: FAIL — `AttributeError: module 'parse' has no attribute 'farming_entries'`.

- [ ] **Step 3: Write the implementation**

Append to `parse.py`:

```python
def _xp_field(fields: dict[str, str], name: str, page: str) -> str | None:
    """Formatted value for one xp field, or None when absent/unparseable.

    A field that is present but does not parse (the wiki writes "N/A" in a
    few Farming rows) is recorded rather than silently dropped.
    """
    raw = fields.get(name)
    if raw is None:
        return None
    value = parse_number(raw)
    if value is None:
        UNPARSEABLE.append(f"{page}: {name}={raw!r}")
        return None
    return _format_xp(value)


def farming_entries(wikitext: str) -> list[tuple[str, str]]:
    """(key, value) from {{Farming info}}.

    Value is plantxp, checkxp and harvestxp -- whichever are present and
    parseable -- joined by "-", matching the section's existing convention.
    """
    fields = extract_template(wikitext, "Farming info")
    if not fields:
        return []
    name = fields.get("name", "").strip()
    if not name:
        return []
    parts = [
        v
        for v in (
            _xp_field(fields, "plantxp", name),
            _xp_field(fields, "checkxp", name),
            _xp_field(fields, "harvestxp", name),
        )
        if v is not None
    ]
    if not parts:
        return []
    return [(_key(name), "-".join(parts))]


def hunter_entries(wikitext: str) -> list[tuple[str, str]]:
    """(key, value) from {{Hunter info}}.

    Value is xp, or "xp/wildxp" when the creature has a distinct Wilderness
    rate -- which in practice is the implings.
    """
    fields = extract_template(wikitext, "Hunter info")
    if not fields:
        return []
    name = fields.get("name", "").strip()
    if not name:
        return []
    xp = _xp_field(fields, "xp", name)
    if xp is None:
        return []
    wild = _xp_field(fields, "wildxp", name)
    return [(_key(name), f"{xp}/{wild}" if wild is not None else xp)]
```

- [ ] **Step 4: Run the full suite**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest -v`
Expected: PASS, 48 passed (40 from Task 1 plus 8 new).

- [ ] **Step 5: Verify against real fetched pages**

Run:

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import fetch, parse
for t in ['Apple tree', 'Willow tree', 'Ranarr weed', 'Watermelon']:
    print(t, '->', parse.farming_entries(fetch.fetch_raw(t)))
for t in ['Baby impling', 'Barb-tailed kebbit', 'Black chinchompa', 'Herbiboar']:
    print(t, '->', parse.hunter_entries(fetch.fetch_raw(t)))
print('UNPARSEABLE:', parse.UNPARSEABLE)
"
```

Expected: `Apple tree -> [('Apple_tree', '22-1199.5-8.5')]` and `Baby impling -> [('Baby_impling', '18/20')]`, matching the values already in the INI. Report exactly what the other six print — `Herbiboar` is expected to differ from the INI, which carries a level-scaled range (`1950-2461`) that the template does not express; that is Task 7's problem, not a parser bug.

**No commit.** Throwaway.

---

### Task 3: Smithing parser

Smithing has no dedicated template. Values come from `{{Recipe}}` blocks whose skill is Smithing. A page may carry several such blocks.

**Files:**
- Modify: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/parse.py`
- Test: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/test_parse.py`

**Interfaces:**
- Consumes: `parse.extract_template`, `parse.parse_number`, `parse._format_xp`, `parse._key`, `parse.UNPARSEABLE`.
- Produces: `smithing_entries(wikitext: str) -> list[tuple[str, str]]`.

`extract_template` finds only the FIRST matching template, so this task needs its own scanner to reach every `{{Recipe}}` block on a page.

- [ ] **Step 1: Write the failing test**

Add to `test_parse.py`:

```python
RECIPE_SINGLE = """{{Recipe
|skill1 = Smithing
|skill1lvl = 99
|skill1exp = 375
|mat1 = Runite bar
|mat1quantity = 5
|output1 = Rune platebody
}}
"""

RECIPE_SECONDARY_SKILL = """{{Recipe
|skill1 = Crafting
|skill1exp = 50
|skill2 = Smithing
|skill2exp = 30
|output1 = Gold bracelet
}}
"""

RECIPE_NOT_SMITHING = """{{Recipe
|skill1 = Cooking
|skill1exp = 30
|output1 = Shrimps
}}
"""

RECIPE_TWO_BLOCKS = """{{Recipe
|skill1 = Smithing
|skill1exp = 50
|output1 = Bronze dagger
}}
Some prose between the blocks.
{{Recipe
|skill1 = Smithing
|skill1exp = 62.5
|output1 = Bronze sword
}}
"""


def test_smithing_reads_output_and_exp():
    assert parse.smithing_entries(RECIPE_SINGLE) == [("Rune_platebody", "375")]


def test_smithing_finds_skill_in_any_slot():
    # Smithing is sometimes the secondary skill; the exp must come from the
    # MATCHING index, not from skill1exp.
    assert parse.smithing_entries(RECIPE_SECONDARY_SKILL) == [
        ("Gold_bracelet", "30")
    ]


def test_smithing_ignores_recipes_for_other_skills():
    assert parse.smithing_entries(RECIPE_NOT_SMITHING) == []


def test_smithing_reads_every_recipe_block_on_the_page():
    assert parse.smithing_entries(RECIPE_TWO_BLOCKS) == [
        ("Bronze_dagger", "50"),
        ("Bronze_sword", "62.5"),
    ]


def test_smithing_returns_empty_without_template():
    assert parse.smithing_entries("Just prose.\n") == []
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest test_parse.py -v`
Expected: FAIL — `AttributeError: module 'parse' has no attribute 'smithing_entries'`.

- [ ] **Step 3: Write the implementation**

Append to `parse.py`:

```python
def _all_template_blocks(wikitext: str, name: str) -> list[str]:
    """Every occurrence of the named template, as raw wikitext blocks.

    extract_template returns only the first match; Smithing pages often carry
    several {{Recipe}} blocks for different methods.
    """
    blocks: list[str] = []
    pattern = re.compile(r"\{\{\s*" + re.escape(name) + r"\s*[\|\n}]")
    pos = 0
    while True:
        match = pattern.search(wikitext, pos)
        if not match:
            return blocks
        start = match.start()
        depth = 0
        i = start
        end = None
        while i < len(wikitext) - 1:
            pair = wikitext[i:i + 2]
            if pair == "{{":
                depth += 1
                i += 2
                continue
            if pair == "}}":
                depth -= 1
                i += 2
                if depth == 0:
                    end = i
                    break
                continue
            i += 1
        if end is None:
            return blocks
        blocks.append(wikitext[start:end])
        pos = end


def smithing_entries(wikitext: str) -> list[tuple[str, str]]:
    """(key, value) for every {{Recipe}} block whose skill is Smithing.

    Keyed by the matching output, valued by the experience of the skill slot
    that names Smithing -- which is not always slot 1.
    """
    out: list[tuple[str, str]] = []
    for block in _all_template_blocks(wikitext, "Recipe"):
        fields = extract_template(block, "Recipe")
        if not fields:
            continue
        index = None
        for n in range(1, 6):
            if fields.get(f"skill{n}", "").strip().lower() == "smithing":
                index = n
                break
        if index is None:
            continue
        name = fields.get("output1", "").strip()
        if not name:
            continue
        raw = fields.get(f"skill{index}exp")
        if raw is None:
            continue
        value = parse_number(raw)
        if value is None:
            UNPARSEABLE.append(f"{name}: skill{index}exp={raw!r}")
            continue
        out.append((_key(name), _format_xp(value)))
    return out
```

- [ ] **Step 4: Run the full suite**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest -v`
Expected: PASS, 53 passed (48 from Task 2 plus 5 new).

- [ ] **Step 5: Verify against real fetched pages**

Run:

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import fetch, parse
for t in ['Rune platebody', 'Bronze dagger', 'Cannonball', 'Steel bar', 'Dragon platebody']:
    print(t, '->', parse.smithing_entries(fetch.fetch_raw(t)))
"
```

Expected: `Rune platebody -> [('Rune_platebody', '375')]`. Report what each prints. If a page returns `[]` that you expected to have a Smithing recipe, investigate before continuing — do not work around it downstream.

**No commit.** Throwaway.

---

### Task 4: Generate the three reconcile reports

**Files:**
- Modify: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/report.py`
- Create (output): `report-smithing.txt`, `report-farming.txt`, `report-hunter.txt`, `wiki-smithing-keys.json`, `wiki-farming-keys.json`, `wiki-hunter-keys.json` in the same directory
- Read only: `rust-reinze/lib/Database.ini`

**Interfaces:**
- Consumes: everything from Tasks 1–3, plus the existing `fetch.list_transclusions`, `fetch.fetch_raw`, `reconcile.read_section`, `reconcile.reconcile`, and `report.write_report`.
- Produces: the six output files, read by Tasks 5, 6 and 7.

No unit test of its own — this is an integration run whose reports are the deliverable. Its gate is the Step 3 sanity checks. Do not modify the existing 53 tests, which must still pass.

- [ ] **Step 1: Add the three gather functions**

Add to `report.py`, following the shape of the existing `gather_construction`. Each must use the same ambiguity discipline: build `key -> list of (page_title, value)`, and when one key is produced by several pages with DIFFERENT values, withhold it from the data entirely and record it for a human. Same-key-same-value is harmless — keep one, record nothing.

```python
def _gather(template: str, extractor) -> tuple[dict[str, str], list[str], list[str]]:
    """Shared sweep: enumerate pages, extract entries, withhold ambiguities.

    Returns (data, ambiguous, no_template_pages). A key produced by several
    pages with DIFFERENT values is withheld from `data` entirely and reported,
    never resolved by last-write-wins.
    """
    titles = fetch.list_transclusions(template)
    contributions: dict[str, list[tuple[str, str]]] = {}
    empty: list[str] = []
    for i, title in enumerate(titles, 1):
        if i % 100 == 0:
            print(f"  {template} {i}/{len(titles)}", file=sys.stderr)
        entries = extractor(fetch.fetch_raw(title))
        if not entries:
            empty.append(title)
            continue
        for key, value in entries:
            contributions.setdefault(key, []).append((title, value))

    data: dict[str, str] = {}
    ambiguous: list[str] = []
    for key, contribs in contributions.items():
        values = {v for _, v in contribs}
        if len(values) == 1:
            data[key] = contribs[0][1]
        else:
            detail = ", ".join(f"{t!r}={v}" for t, v in sorted(contribs))
            ambiguous.append(f"{key}: {detail}")
    return data, ambiguous, empty
```

Then the three callers:

```python
def gather_smithing():
    return _gather("Template:Recipe", parse.smithing_entries)


def gather_farming():
    return _gather("Template:Farming info", parse.farming_entries)


def gather_hunter():
    return _gather("Template:Hunter info", parse.hunter_entries)
```

- [ ] **Step 2: Extend main() to produce the three reports**

Add this driver and call it once per section from `main()`. The `UNPARSEABLE` snapshot matters: that list accumulates across the whole run, so slicing from a pre-gather mark is what stops Farming's `N/A` rows appearing in Smithing's report.

```python
def run_section(section: str, template: str, extractor, ini_text: str) -> None:
    """Gather one section, reconcile it, and write its report and key dump."""
    mark = len(parse.UNPARSEABLE)
    print(f"gathering {section}...", file=sys.stderr)
    data, ambiguous, empty = _gather(template, extractor)
    unparseable = parse.UNPARSEABLE[mark:]

    buckets = reconcile.reconcile(reconcile.read_section(ini_text, section), data)

    lines = [f"=== {section} reconcile report ===", ""]
    lines.append(f"unchanged: {len(buckets['unchanged'])} entries (not listed)")
    lines.append("")
    lines.append(f"--- CHANGED ({len(buckets['changed'])}) : ini -> wiki")
    for key, have, want in sorted(buckets["changed"]):
        lines.append(f"  {key}: {have} -> {want}")
    lines.append("")
    lines.append(f"--- WIKI-ONLY / candidate additions ({len(buckets['wiki_only'])})")
    for key, value in buckets["wiki_only"]:
        lines.append(f"  {key}={value}")
    lines.append("")
    lines.append(f"--- INI-ONLY / decide each by hand ({len(buckets['ini_only'])})")
    for key, value in sorted(buckets["ini_only"]):
        lines.append(f"  {key}={value}")
    lines.append("")
    lines.append(f"--- AMBIGUOUS KEYS, multiple pages disagree ({len(ambiguous)})")
    for entry in sorted(ambiguous):
        lines.append(f"  {entry}")
    lines.append("")
    lines.append(f"--- CASE COLLISIONS ({len(buckets['collisions'])})")
    for entry in buckets["collisions"]:
        lines.append(f"  {entry}")
    lines.append("")
    lines.append(f"--- UNPARSEABLE FIELDS ({len(unparseable)})")
    for entry in unparseable:
        lines.append(f"  {entry}")
    lines.append("")
    lines.append(f"--- PAGES WITH NO RELEVANT TEMPLATE ({len(empty)})")
    for title in empty:
        lines.append(f"  {title}")
    lines.append("")

    slug = section.lower()
    (HERE / f"report-{slug}.txt").write_text("\n".join(lines), encoding="utf-8")
    (HERE / f"wiki-{slug}-keys.json").write_text(
        json.dumps(sorted(data), indent=1), encoding="utf-8"
    )
    print(f"wrote report-{slug}.txt ({len(data)} wiki keys)")
```

Then in `main()`, after reading the INI once:

```python
    ini_text = INI_PATH.read_text(encoding="utf-8")
    run_section("Smithing", "Template:Recipe", parse.smithing_entries, ini_text)
    run_section("Farming", "Template:Farming info", parse.farming_entries, ini_text)
    run_section("Hunter", "Template:Hunter info", parse.hunter_entries, ini_text)
```

`json` must be imported at the top of `report.py` rather than inside a function. Leave the existing Prayer and Construction code in the file, but do not call it from `main()` any more — those sections are already committed and re-running the 522-page Construction sweep wastes several minutes. Deleting that code is also fine; it is throwaway either way.

- [ ] **Step 3: Run it and sanity-check**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u report.py 2>report.stderr.log`

The Smithing sweep fetches roughly 3,700 pages on a cold cache — allow ten minutes or more, and use a generous timeout. Do NOT reduce the page count to make it finish faster.

Then verify:

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import json, pathlib, re
for s in ('smithing','farming','hunter'):
    t = pathlib.Path(f'report-{s}.txt').read_text()
    keys = json.loads(pathlib.Path(f'wiki-{s}-keys.json').read_text())
    counts = dict(re.findall(r'--- ([A-Z /,-]+?) \((\d+)\)', t))
    print(s, '| wiki keys:', len(keys), '|', counts)
"
```

Expected and required:
- Farming wiki keys in the low hundreds (the template has 255 transclusions; some are non-crop pages).
- Hunter wiki keys near 89.
- Smithing wiki keys in the hundreds — this is the filtered subset of 3,724 recipes.
- Every section's `unchanged + changed + wiki_only` equals its wiki key count. If it does not, ambiguous keys are leaking into the data; stop and fix.
- Farming UNPARSEABLE must contain the four `N/A` fields (Belladonna, Bittercap mushroom, Evil turnip, Nightshade) — these are the `checkxp = N/A` rows.
- Report the exact bucket sizes for all three sections.

- [ ] **Step 4: Read all three reports in full**

Do not proceed to Task 5 without having read `report-smithing.txt`, `report-farming.txt` and `report-hunter.txt`. Tasks 5–7 are hand edits driven entirely by their contents.

**No commit.** Throwaway.

---

### Task 5: Rewrite the `[Smithing]` section

**Files:**
- Modify: `rust-reinze/lib/Database.ini` — the `[Smithing]` section only
- Read: `report-smithing.txt`, `wiki-smithing-keys.json`

**Interfaces:**
- Consumes: Task 4's Smithing report.
- Produces: nothing consumed by later tasks. Tasks 6 and 7 are independent.

**Rules:**

- One alphabetically sorted block. Keys are the wiki's exact item name with spaces replaced by `_`. Values are bare numbers.
- **Screen every addition for alpha/beta content:** reject any page carrying `{{Sailing Beta Content}}`, `{{Beta}}`, or an `|id =` beginning `beta`. This caught `Monodon bones` in the Prayer audit. Screen on the marker, never on whether an item looks familiar — recency reads identically to unfamiliarity.
- **Watch for alternate-method contamination.** Giants' Foundry and Blast Furnace award different XP for the same item. Any key produced by several pages with different values is in the AMBIGUOUS bucket; resolve each by hand, keying off the page title and keeping both sides when they are genuinely distinct items. Do not let a minigame rate overwrite the standard one — this is what nearly corrupted ~24 Construction values.
- **Deduplicate `4_Cannon_Balls` and `4_Cannonballs`** (both `25.6`) to the wiki's name.
- Every INI-ONLY entry is decided individually — keep, rename, correct, or drop. Drop ONLY as a duplicate, superseded, or demonstrably not Old School.

- [ ] **Step 1: Write out every INI-ONLY disposition before editing**

Print your disposition for each INI-ONLY entry: keep / rename to X / correct to N / drop because Y. This is the core of the task.

- [ ] **Step 2: Spot-check ten CHANGED values**

Fetch each item's own page and read its `{{Recipe}}` block:

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import fetch, parse
for t in ['Rune platebody','Adamant platebody','Mithril platebody','Steel platebody',
          'Iron platebody','Bronze platebody','Rune scimitar','Cannonball',
          'Gold bar','Steel bar']:
    print(t, parse.smithing_entries(fetch.fetch_raw(t)))
"
```

Replace any that are not in the CHANGED bucket with ones that are — the point is to check values the tool says are changing.

- [ ] **Step 3: Apply the edit**

Use `Edit` on `rust-reinze/lib/Database.ini`, replacing the whole `[Smithing]` section.

- [ ] **Step 4: Run the mechanical scan**

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && python3 - <<'EOF'
import json, pathlib, re
lines = open('lib/Database.ini').read().split('\n')
i = lines.index('[Smithing]')
rows = []
for l in lines[i+1:]:
    if l.startswith('['): break
    if l.strip(): rows.append(l)
keys = [r.split('=',1)[0] for r in rows]
print('entries:', len(rows))
assert all('=' in r for r in rows)
assert not [r for r in rows if r.endswith('=')], 'empty value'
assert not [k for k in keys if ' ' in k], 'space in key'
assert not [k for k in keys if re.search(r'<|\[\[|\]\]|&#', k)], 'markup in key'
assert not [r for r in rows if not re.fullmatch(r'\d+(\.\d+)?', r.split('=',1)[1])], \
    'value not a bare number: %s' % [r for r in rows if not re.fullmatch(r'\d+(\.\d+)?', r.split('=',1)[1])][:5]
low = [k.lower() for k in keys]
assert len(low) == len(set(low)), 'duplicate key'
assert keys == sorted(keys, key=str.lower), 'not sorted'
wiki = set(json.loads(pathlib.Path(
    '/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/wiki-smithing-keys.json').read_text()))
KEPT = {
    # Hand-kept entries with no qualifying Recipe block. Fill in from your
    # Step 1 dispositions, one comment per entry naming the page you checked.
}
stray = [k for k in keys if k not in wiki and k not in KEPT]
assert not stray, 'keys neither wiki names nor justified: %s' % stray
print('Smithing section clean:', len(rows), 'entries')
EOF
```

- [ ] **Step 5: Validate with the real `ini` crate**

The plugin's own tests never load the INI, so `cargo test` cannot catch a broken data file. Create `reinze-lib-runescape/examples/ini_smoke.rs`:

```rust
// Throwaway INI validator. Created, run, and deleted within one task.
use ini::Ini;

fn main() {
    let path = std::env::args().nth(1).expect("usage: ini_smoke <path>");
    let ini = Ini::load_from_file(&path).expect("Ini::load_from_file failed");
    for section in ["Smithing", "Farming", "Hunter", "Prayer", "Construction"] {
        let props = ini
            .section(Some(section))
            .unwrap_or_else(|| panic!("missing section [{section}]"));
        let mut lower: Vec<String> =
            props.iter().map(|(k, _)| k.to_ascii_lowercase()).collect();
        let total = lower.len();
        lower.sort();
        lower.dedup();
        assert_eq!(total, lower.len(), "[{section}] key collision");
        for (k, v) in props.iter() {
            assert!(!v.trim().is_empty(), "[{section}] {k} has an empty value");
            assert!(!k.contains(' '), "[{section}] {k} contains a space");
        }
        println!("[{section}] {total} entries");
    }
    println!("INI OK");
}
```

Run it and the plugin suite:

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape \
  && cargo run --quiet --example ini_smoke -- \
     /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze/lib/Database.ini \
  && cargo test --quiet 2>&1 | tail -5
```

Expected: a line per section then `INI OK`, then `test result: ok`.

- [ ] **Step 6: Delete the example and confirm the plugin repo is clean**

```bash
rm /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples/ini_smoke.rs
rmdir /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples 2>/dev/null
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && git status --short
```

Expected: exactly `?? .idea/`, `?? docs/`, `?? sql/` and nothing else.

- [ ] **Step 7: Confirm nothing else moved**

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze \
  && git diff HEAD -- lib/Database.ini | grep -E '^[-+]\[[A-Za-z]+\]' || echo "no section header changed"
```

Expected: prints the "no section header changed" fallback.

- [ ] **Step 8: Commit**

The body is written from the report. It must contain, each as its own paragraph wrapped at 72 columns: `Smithing (259 -> N).` and what was wrong; what was added, grouped rather than enumerated; what was renamed, including the cannonball deduplication; what was dropped and why; and anything deliberately left alone. Read `git show 330b941` and `git show 93ea090` first and match their voice.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && git add lib/Database.ini && git commit -m "$(cat <<'EOF'
chore(data): Update Smithing params from wiki

Smithing (259 -> N). ...

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)" && git show --stat HEAD | tail -3
```

Expected: `1 file changed`, `lib/Database.ini`.

---

### Task 6: Rewrite the `[Farming]` section

**Files:**
- Modify: `rust-reinze/lib/Database.ini` — the `[Farming]` section only
- Read: `report-farming.txt`, `wiki-farming-keys.json`

**Interfaces:**
- Consumes: Task 4's Farming report.
- Produces: nothing.

**Rules:**

- One alphabetically sorted block. Keys are the wiki's exact plant name with spaces replaced by `_`.
- **Values keep the composite encoding**: `plantxp`, `checkxp` and `harvestxp` joined by `-`, omitting fields the plant does not have. `Apple_tree=22-1199.5-8.5`, `Oak_tree=14-467.3`, `Asgarnian_hops=10.5-12`.
- **No `N/A` may survive.** Four entries currently carry one: `Belladonna=92-N/A-512`, `Bittercap_Mushroom=61.5-N/A-57.7`, `Evil_Turnip=41-N/A-46`, `Nightshade=92-N/A-512`. The wiki writes `checkxp = N/A` for these. Drop the field from the composite so they become two-part values, unless the plant's page gives a real number — check each.
- **Keys move to wiki plant names.** `Acorn_Oak_Tree` becomes `Oak_tree`, so a query for `acorn` no longer finds it. This was accepted deliberately; note it in the commit body.
- Screen every addition for alpha/beta content exactly as in Task 5.
- Every INI-ONLY entry decided individually; drop only as a duplicate, superseded, or demonstrably not Old School.

- [ ] **Step 1: Write out every INI-ONLY disposition before editing**

Print keep / rename to X / correct to N / drop because Y for each.

- [ ] **Step 2: Resolve the four `N/A` entries against their pages**

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import fetch, parse
for t in ['Belladonna','Bittercap mushroom','Evil turnip','Nightshade']:
    f = parse.extract_template(fetch.fetch_raw(t), 'Farming info')
    print(t, '->', {k: f.get(k) for k in ('name','plantxp','checkxp','harvestxp')} if f else None)
"
```

State what each resolves to and why.

- [ ] **Step 3: Apply the edit**

Use `Edit` on `rust-reinze/lib/Database.ini`, replacing the whole `[Farming]` section.

- [ ] **Step 4: Run the mechanical scan**

Note the value grammar differs from Smithing's — up to three `-`-joined numbers.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && python3 - <<'EOF'
import json, pathlib, re
lines = open('lib/Database.ini').read().split('\n')
i = lines.index('[Farming]')
rows = []
for l in lines[i+1:]:
    if l.startswith('['): break
    if l.strip(): rows.append(l)
keys = [r.split('=',1)[0] for r in rows]
vals = [r.split('=',1)[1] for r in rows]
print('entries:', len(rows))
assert not [r for r in rows if r.endswith('=')], 'empty value'
assert not [k for k in keys if ' ' in k], 'space in key'
assert not [k for k in keys if re.search(r'<|\[\[|\]\]|&#', k)], 'markup in key'
assert not [v for v in vals if 'N/A' in v], 'N/A survives: %s' % [v for v in vals if 'N/A' in v]
bad = [r for r in rows if not re.fullmatch(r'\d+(\.\d+)?(-\d+(\.\d+)?){0,2}', r.split('=',1)[1])]
assert not bad, 'value does not match the Farming grammar: %s' % bad[:5]
low = [k.lower() for k in keys]
assert len(low) == len(set(low)), 'duplicate key'
assert keys == sorted(keys, key=str.lower), 'not sorted'
wiki = set(json.loads(pathlib.Path(
    '/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/wiki-farming-keys.json').read_text()))
KEPT = {
    # Hand-kept entries with no Farming info page. Fill in from Step 1,
    # one comment per entry naming the page you checked.
}
stray = [k for k in keys if k not in wiki and k not in KEPT]
assert not stray, 'keys neither wiki names nor justified: %s' % stray
print('Farming section clean:', len(rows), 'entries')
EOF
```

- [ ] **Step 5: Validate with the real `ini` crate**

Create `reinze-lib-runescape/examples/ini_smoke.rs` with exactly this content:

```rust
// Throwaway INI validator. Created, run, and deleted within one task.
use ini::Ini;

fn main() {
    let path = std::env::args().nth(1).expect("usage: ini_smoke <path>");
    let ini = Ini::load_from_file(&path).expect("Ini::load_from_file failed");
    for section in ["Smithing", "Farming", "Hunter", "Prayer", "Construction"] {
        let props = ini
            .section(Some(section))
            .unwrap_or_else(|| panic!("missing section [{section}]"));
        let mut lower: Vec<String> =
            props.iter().map(|(k, _)| k.to_ascii_lowercase()).collect();
        let total = lower.len();
        lower.sort();
        lower.dedup();
        assert_eq!(total, lower.len(), "[{section}] key collision");
        for (k, v) in props.iter() {
            assert!(!v.trim().is_empty(), "[{section}] {k} has an empty value");
            assert!(!k.contains(' '), "[{section}] {k} contains a space");
        }
        println!("[{section}] {total} entries");
    }
    println!("INI OK");
}
```

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape \
  && cargo run --quiet --example ini_smoke -- \
     /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze/lib/Database.ini \
  && cargo test --quiet 2>&1 | tail -5
```

Expected: a line per section, `INI OK`, `test result: ok`.

- [ ] **Step 6: Delete the example and confirm the plugin repo is clean**

```bash
rm /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples/ini_smoke.rs
rmdir /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples 2>/dev/null
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && git status --short
```

Expected: exactly `?? .idea/`, `?? docs/`, `?? sql/`.

- [ ] **Step 7: Confirm only Farming moved**

The Smithing commit is already `HEAD`, so diffing against `HEAD` shows this task alone:

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze \
  && git diff HEAD -- lib/Database.ini | grep -E '^[-+]\[[A-Za-z]+\]' || echo "no section header changed"
```

- [ ] **Step 8: Commit**

Body paragraphs, wrapped at 72 columns: `Farming (68 -> N).` and what was wrong; the four `N/A` values and how each resolved; what was added; the move to wiki plant names and that `acorn` no longer finds the oak tree; what was dropped and why; what was left alone. Match the voice of `git show 93ea090`.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && git add lib/Database.ini && git commit -m "$(cat <<'EOF'
chore(data): Update Farming params from wiki

Farming (68 -> N). ...

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)" && git show --stat HEAD | tail -3
```

Expected: `1 file changed`, `lib/Database.ini`.

---

### Task 7: Rewrite the `[Hunter]` section

**Files:**
- Modify: `rust-reinze/lib/Database.ini` — the `[Hunter]` section only
- Read: `report-hunter.txt`, `wiki-hunter-keys.json`

**Interfaces:**
- Consumes: Task 4's Hunter report.
- Produces: nothing. This is the last task.

**Rules:**

- One alphabetically sorted block. Keys are the wiki's exact creature name with spaces replaced by `_`. Four keys legitimately contain a hyphen and must keep it: `Barb-tailed_kebbit`, `Razor-backed_kebbit`, `Sabre-toothed_kebbit`, `Sabre-toothed_kyatt`.
- **Two separators with different meanings, both preserved:**
  - `/` is `xp/wildxp` — the normal and Wilderness rates, which in practice is the eleven implings (`Baby_impling=18/20`).
  - `-` is a level-scaled XP range, used by exactly two entries: `Herbiboar=1950-2461` and `Wyrmscraig_goat=100-179`.
- **`{{Hunter info}}` does not express the level-scaled range.** So `Herbiboar` and `Wyrmscraig_goat` will appear in the CHANGED bucket with a single value that looks like a correction but is not. **Keep their range form.** Verify both endpoints against their own wiki pages and put both keys in the `KEPT` allowlist with a comment. This is the same class of false correction as the Mahogany Homes collision in the Construction audit.
- Screen every addition for alpha/beta content exactly as in Task 5.
- Every INI-ONLY entry decided individually; drop only as a duplicate, superseded, or demonstrably not Old School.

- [ ] **Step 1: Write out every INI-ONLY disposition before editing**

Print keep / rename to X / correct to N / drop because Y for each.

- [ ] **Step 2: Verify the two range entries against their pages**

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import fetch, re
for t in ['Herbiboar','Wyrmscraig goat']:
    w = fetch.fetch_raw(t)
    print('=====', t)
    for l in w.split(chr(10)):
        if re.search(r'xp|experience', l, re.I) and 'image' not in l.lower():
            print('   ', l.strip()[:150])
"
```

State what the endpoints are and whether the INI's current `1950-2461` and `100-179` are right.

- [ ] **Step 3: Apply the edit**

Use `Edit` on `rust-reinze/lib/Database.ini`, replacing the whole `[Hunter]` section.

- [ ] **Step 4: Run the mechanical scan**

Note the Hunter grammar allows one `/` OR one `-` separator.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && python3 - <<'EOF'
import json, pathlib, re
lines = open('lib/Database.ini').read().split('\n')
i = lines.index('[Hunter]')
rows = []
for l in lines[i+1:]:
    if l.startswith('['): break
    if l.strip(): rows.append(l)
keys = [r.split('=',1)[0] for r in rows]
print('entries:', len(rows))
assert not [r for r in rows if r.endswith('=')], 'empty value'
assert not [k for k in keys if ' ' in k], 'space in key'
assert not [k for k in keys if re.search(r'<|\[\[|\]\]|&#', k)], 'markup in key'
bad = [r for r in rows if not re.fullmatch(r'\d+(\.\d+)?([/-]\d+(\.\d+)?)?', r.split('=',1)[1])]
assert not bad, 'value does not match the Hunter grammar: %s' % bad[:5]
low = [k.lower() for k in keys]
assert len(low) == len(set(low)), 'duplicate key'
assert keys == sorted(keys, key=str.lower), 'not sorted'
ranged = [r for r in rows if '-' in r.split('=',1)[1]]
assert sorted(r.split('=',1)[0] for r in ranged) == ['Herbiboar', 'Wyrmscraig_goat'], \
    'unexpected range entries: %s' % ranged
wiki = set(json.loads(pathlib.Path(
    '/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/wiki-hunter-keys.json').read_text()))
KEPT = {
    'Herbiboar',        # level-scaled range, not expressible in Hunter info
    'Wyrmscraig_goat',  # level-scaled range, not expressible in Hunter info
    # add any further hand-kept entries from Step 1, with a comment each
}
stray = [k for k in keys if k not in wiki and k not in KEPT]
assert not stray, 'keys neither wiki names nor justified: %s' % stray
print('Hunter section clean:', len(rows), 'entries')
EOF
```

- [ ] **Step 5: Validate with the real `ini` crate**

Create `reinze-lib-runescape/examples/ini_smoke.rs` with exactly this content:

```rust
// Throwaway INI validator. Created, run, and deleted within one task.
use ini::Ini;

fn main() {
    let path = std::env::args().nth(1).expect("usage: ini_smoke <path>");
    let ini = Ini::load_from_file(&path).expect("Ini::load_from_file failed");
    for section in ["Smithing", "Farming", "Hunter", "Prayer", "Construction"] {
        let props = ini
            .section(Some(section))
            .unwrap_or_else(|| panic!("missing section [{section}]"));
        let mut lower: Vec<String> =
            props.iter().map(|(k, _)| k.to_ascii_lowercase()).collect();
        let total = lower.len();
        lower.sort();
        lower.dedup();
        assert_eq!(total, lower.len(), "[{section}] key collision");
        for (k, v) in props.iter() {
            assert!(!v.trim().is_empty(), "[{section}] {k} has an empty value");
            assert!(!k.contains(' '), "[{section}] {k} contains a space");
        }
        println!("[{section}] {total} entries");
    }
    println!("INI OK");
}
```

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape \
  && cargo run --quiet --example ini_smoke -- \
     /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze/lib/Database.ini \
  && cargo test --quiet 2>&1 | tail -5
```

- [ ] **Step 6: Delete the example and confirm the plugin repo is clean**

```bash
rm /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples/ini_smoke.rs
rmdir /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples 2>/dev/null
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && git status --short
```

Expected: exactly `?? .idea/`, `?? docs/`, `?? sql/`.

- [ ] **Step 7: Confirm only Hunter moved**

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze \
  && git diff HEAD -- lib/Database.ini | grep -E '^[-+]\[[A-Za-z]+\]' || echo "no section header changed"
```

- [ ] **Step 8: Commit**

Body paragraphs, wrapped at 72 columns: `Hunter (76 -> N).` and what was wrong; what was added; that the two composite encodings were preserved and why Herbiboar and the Wyrmscraig goat keep their level-scaled ranges; what was dropped and why; what was left alone. Match the voice of `git show 93ea090`.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && git add lib/Database.ini && git commit -m "$(cat <<'EOF'
chore(data): Update Hunter params from wiki

Hunter (76 -> N). ...

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)" && git show --stat HEAD | tail -3
```

Expected: `1 file changed`, `lib/Database.ini`.

---

## Final verification

Run every command and paste the output. Do not report completion on any of these being assumed.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze \
  && git log --oneline -5 \
  && for n in 1 2 3; do git show --stat HEAD~$((n-1)) | tail -3; done \
  && git status --short
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && git status --short
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && cargo test --quiet 2>&1 | tail -5
```

- [ ] `git log --oneline -5` shows the three `chore(data):` commits on top of `6fb2740`.
- [ ] `git show --stat` on each shows `lib/Database.ini` as the only file changed.
- [ ] `rust-reinze` `git status --short` shows only the pre-existing untracked entries: `.idea/`, `conf/rizon.toml.example` and the March docs.
- [ ] `reinze-lib-runescape` `git status --short` shows only `.idea/`, `docs/`, `sql/` — no `examples/`.
- [ ] `cargo test` in `reinze-lib-runescape` passes.
- [ ] Nothing from `/home/rohara/.claude/jobs/5422e5c0/tmp/` is committed.
- [ ] The mechanical scans from Tasks 5, 6 and 7 all still pass against the final file.
- [ ] `[Prayer]` (73 entries) and `[Construction]` (483 entries) are byte-identical to `6fb2740`.
