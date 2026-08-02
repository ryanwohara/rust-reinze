# Crafting and Magic params audit, and result ranking — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-derive the `[Crafting]` and `[Magic]` sections of `lib/Database.ini` from the OSRS wiki, and make `+params` rank its ten displayed results by relevance instead of alphabetical accident.

**Architecture:** Part 1 is a self-contained change to `reinze-lib-runescape/src/params.rs`, extracting a pure ranking function so it can be unit-tested without loading the INI. Part 2 reuses the audit tool from the previous two runs, generalising its Smithing recipe parser to any skill and adding a spell parser, then producing two reconcile reports a human works by hand.

**Tech Stack:** Rust (`rust-ini 0.21`, no new crates) for Part 1. Python 3 standard library plus pytest for the throwaway audit tool. No new dependencies anywhere.

**Note for whoever reads this later:** this is the last audit that targets the INI format. `lib/Database.ini` sits in the bot repo only because its runtime load path is cwd-relative, while its sole consumer lives in the plugin repo — which is why every task here creates a validator in one repo to check a file in the other. A migration to per-skill `.rs` files is planned immediately after this plan lands, and will be designed separately. Finishing the audit first means that migration converts a fully verified dataset and can be checked by asserting the parsed maps are identical.

## Global Constraints

- **TWO SEPARATE GIT REPOSITORIES.** `/home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape` (the plugin, holds `params.rs`) and `.../rust-reinze` (the bot, holds `lib/Database.ini`). `rust-reinze` has NO cargo dependency on the plugin — plugins load at runtime as `.so`. So `cargo test -p reinze-lib-runescape` from the bot repo does not work; run plugin tests with `cd reinze-lib-runescape && cargo test`.
- **Branches.** `rust-reinze` work goes on `data/crafting-magic-params`, already created, spec committed as `d2dd619`. `reinze-lib-runescape` is currently on `main` with 1 unpushed commit — Task 1 must create its own branch there before committing.
- **The audit tool** lives at `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/` with a warm cache of 4,200+ pages and **66 passing tests**. Run the whole suite (`python3 -m pytest -v` in that directory) after every change; no existing test may break. The tool is NEVER committed — do not `git add` anything under that path.
- **`conftest.py`** has an autouse fixture redirecting `fetch.CACHE_DIR` to a per-test `tmp_path`. Leave it alone. No new test may touch the real `cache/` or the network.
- **`lib/Database.ini` uses CRLF.** `Edit` preserves it; verify the count after editing, as the previous three data tasks did.
- **Do not touch** any section other than `[Crafting]` and `[Magic]`. `[Prayer]` (73), `[Construction]` (483), `[Smithing]` (358), `[Farming]` (94), `[Hunter]` (86) and the six combat sections plus `[Slayer]` must stay byte-identical.
- **Keys** are the wiki's exact item or spell name with spaces replaced by `_`. Hyphens only where the wiki name genuinely has one. No literal spaces, no `<`, `[[`, `]]`, `&#`, and no `#`.
- **Standing screens on every addition**, established by earlier sections:
  - Alpha/beta: reject `{{Sailing Beta Content}}` **with or without a parameter**, `{{Beta}}`, or an `|id =` beginning `beta`.
  - Removed-from-game: reject `{{Gone}}` or an `|id =` beginning `hist`.
  - **Sanity-check each screen against a known positive before trusting a zero result.** `Monodon bones` trips the beta screen; `Smoker canister` trips the gone screen. The literal-string form of the beta marker misses `{{Sailing Beta Content|alpha}}` — three separate agents have now hit this.
  - Zero-XP rows excluded.
- **Commit style.** `reinze-lib-runescape` uses conventional commits with a scope (`feat(chef):`, `fix(chef):`). `rust-reinze` data commits use `chore(data): Update <Skill> params from wiki`. Bodies wrap at 72 columns and end with `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`. Read `git show 93ea090` and `git show e694896` in `rust-reinze` for the data voice.

**Existing tool interfaces (already built and tested — use, do not modify):**

```python
fetch.list_transclusions(template: str) -> list[str]
fetch.fetch_raw(title: str) -> str
parse.extract_template(wikitext: str, name: str) -> dict[str, str] | None
parse._all_template_blocks(wikitext: str, name: str) -> list[str]
parse._skill_indices(fields: dict[str, str]) -> list[int]   # every skill(\d+), int-sorted
parse.parse_number(raw: str) -> float | None
parse._format_xp(value: float) -> str
parse._key(name: str) -> str
parse._xp_field(fields: dict, name: str, page: str) -> str | None
parse.UNPARSEABLE: list[str]
reconcile.read_section(ini_text: str, section: str) -> dict[str, str]
reconcile.reconcile(ini: dict[str,str], wiki: dict[str,str]) -> dict[str, list]
report._gather(template: str, extractor) -> tuple[dict[str,str], list[str], list[str]]
report.run_section(section: str, template: str, extractor, ini_text: str) -> None
```

---

### Task 1: Rank `+params` results

**Files:**
- Modify: `reinze-lib-runescape/src/params.rs` — the `let found_params: Vec<String> = section.iter().filter(...)` block inside `lookup`, and the `#[cfg(test)]` module

Do not navigate by line number in this task: Step 3 inserts a function above `lookup`, so every line below it shifts. Find the block by its content.

**Interfaces:**
- Consumes: nothing.
- Produces: `pub(crate) fn rank_matches<'a>(keys: &[&'a str], query: &str) -> Vec<&'a str>` — every key matching `query`, ranked, uncapped. The caller applies `.take(10)`.

Ranking is tier, then fewest extra underscore tokens, then case-insensitive alphabetical. Extracting a pure function matters: the two existing tests deliberately return before `Ini::load_from_file`, so the suite never reads the data file, and that property must survive.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `src/params.rs`:

```rust
    #[test]
    fn ranking_puts_an_exact_match_first() {
        let keys = ["Steel_bar", "Bar_magnet", "Bar"];
        assert_eq!(rank_matches(&keys, "bar")[0], "Bar");
    }

    #[test]
    fn ranking_puts_a_prefix_match_above_a_substring() {
        let keys = ["Steel_bar", "Bar_magnet"];
        assert_eq!(rank_matches(&keys, "bar"), vec!["Bar_magnet", "Steel_bar"]);
    }

    #[test]
    fn ranking_prefers_fewer_extra_tokens() {
        // The plain cannonballs must outrank the chainshot and incendiary
        // variants; this is the regression that motivated the change.
        let keys = [
            "Adamant_chainshot_cannonball",
            "Steel_cannonball",
            "Bronze_incendiary_cannonball",
            "Rune_cannonball",
        ];
        assert_eq!(
            rank_matches(&keys, "cannonball"),
            vec![
                "Rune_cannonball",
                "Steel_cannonball",
                "Adamant_chainshot_cannonball",
                "Bronze_incendiary_cannonball",
            ]
        );
    }

    #[test]
    fn ranking_keeps_peers_alphabetical() {
        // A list of same-shaped matches must not be reshuffled.
        let keys = ["Camelot_Teleport", "Annakarl_Teleport", "Ardougne_Teleport"];
        assert_eq!(
            rank_matches(&keys, "teleport"),
            vec!["Annakarl_Teleport", "Ardougne_Teleport", "Camelot_Teleport"]
        );
    }

    #[test]
    fn ranking_treats_spaces_in_the_query_as_underscores() {
        let keys = ["Oak_bird_house", "Bird_house", "Birdsong"];
        assert_eq!(
            rank_matches(&keys, "bird house"),
            vec!["Bird_house", "Oak_bird_house"]
        );
    }

    #[test]
    fn ranking_is_case_insensitive_both_ways() {
        let keys = ["GOLD_BAR", "gold_ore"];
        assert_eq!(rank_matches(&keys, "GoLd_BaR"), vec!["GOLD_BAR"]);
    }

    #[test]
    fn ranking_returns_nothing_when_no_key_matches() {
        let keys = ["Gold_bar", "Iron_bar"];
        assert!(rank_matches(&keys, "dragon").is_empty());
    }

    #[test]
    fn ranking_returns_nothing_for_an_empty_query() {
        let keys = ["Gold_bar"];
        assert!(rank_matches(&keys, "").is_empty());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && cargo test --quiet params 2>&1 | tail -20`
Expected: compile error, `cannot find function 'rank_matches' in this scope`.

- [ ] **Step 3: Write the ranking function**

Add above `pub fn lookup` in `src/params.rs`:

```rust
/// Every key matching `query`, best first. The caller caps the count.
///
/// Ranked by tier (exact, then prefix, then substring), then by fewest extra
/// underscore-separated tokens, then case-insensitively alphabetically. The
/// token count is what lifts a buried canonical answer above its longer
/// variants; the alphabetical tiebreak is what stops a list of peers being
/// reshuffled.
pub(crate) fn rank_matches<'a>(keys: &[&'a str], query: &str) -> Vec<&'a str> {
    let needle = query.replace(' ', "_").to_ascii_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let needle_tokens = needle.split('_').count();

    let mut scored: Vec<(u8, usize, String, &'a str)> = keys
        .iter()
        .filter_map(|key| {
            let lower = key.to_ascii_lowercase();
            let tier = if lower == needle {
                0
            } else if lower.starts_with(&needle) {
                1
            } else if lower.contains(&needle) {
                2
            } else {
                return None;
            };
            let extra = lower.split('_').count().saturating_sub(needle_tokens);
            Some((tier, extra, lower, *key))
        })
        .collect();

    scored.sort();
    scored.into_iter().map(|(_, _, _, key)| key).collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && cargo test --quiet params 2>&1 | tail -10`
Expected: PASS. 10 tests in the params module (the 2 existing plus your 8).

- [ ] **Step 5: Wire it into `lookup`**

Replace the `let found_params: Vec<String> = section.iter().filter(...).take(10).map(...).collect();` block in `lookup` — located by content, since Step 3 shifted the line numbers — with:

```rust
    let keys: Vec<&str> = section.iter().map(|(k, _)| k).collect();

    let found_params: Vec<String> = rank_matches(&keys, param)
        .into_iter()
        .take(10)
        .map(|k| {
            let v = section.get(k).unwrap_or("");
            format!(
                "{} {}",
                s.c1(&k.replace("_", " ")),
                s.c2(&format!("{}xp", v))
            )
        })
        .collect();
```

The `let underscored = param.replace(" ", "_");` line above it becomes dead — `rank_matches` does that itself. Delete it.

- [ ] **Step 6: Run the whole crate suite**

Run: `cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && cargo test --quiet 2>&1 | tail -5`
Expected: `test result: ok`, 187 passed (179 existing plus your 8). No warnings about an unused `underscored` binding.

- [ ] **Step 7: Confirm the behaviour against the real data file**

The permanent suite never loads the INI, so verify the ranking end-to-end once with a **temporary test inside the module**. It must be a test, not an example: `params` is a private module (`mod params;` in `src/lib.rs`) and the crate is `crate-type = ["dylib"]`, so an example cannot link to `rank_matches` at all — and a copy-pasted duplicate of the logic would prove nothing, since it could pass while the real function differs.

Add this to the `mod tests` block temporarily:

```rust
    #[test]
    fn ctl_ranking_against_the_real_database() {
        let path = "/home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze/lib/Database.ini";
        let ini = Ini::load_from_file(path).expect("load failed");
        for (section, query) in [
            ("Smithing", "cannonball"),
            ("Smithing", "bar"),
            ("Hitpoints", "goblin"),
            ("Magic", "teleport"),
        ] {
            let props = ini.section(Some(section)).expect("missing section");
            let keys: Vec<&str> = props.iter().map(|(k, _)| k).collect();
            let needle = query.replace(' ', "_").to_ascii_lowercase();
            let total = keys
                .iter()
                .filter(|k| k.to_ascii_lowercase().contains(&needle))
                .count();
            let top: Vec<&str> = rank_matches(&keys, query).into_iter().take(5).collect();
            println!("[{section}] {query:?} — {total} matches, top 5: {top:?}");
        }
    }
```

Run it with output shown:

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape \
  && cargo test --quiet ctl_ranking_against_the_real_database -- --nocapture 2>&1 | tail -12
```

Expected: `cannonball` shows plain cannonballs first, not `Adamant_chainshot_cannonball`; `bar` no longer leads with `'perfect'_gold_bar`; `goblin` leads with `Goblin_*` rather than `Angry_goblin_*`; `teleport` stays alphabetical apart from `Teleport_Catherby` being promoted as a genuine prefix match. Paste the output.

- [ ] **Step 8: Delete the temporary test and confirm only the intended change remains**

Remove `ctl_ranking_against_the_real_database` entirely — it must not ship, because it would make the suite depend on a file in another repository.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape \
  && grep -c 'ctl_ranking_against_the_real_database' src/params.rs \
  && cargo test --quiet 2>&1 | tail -5 \
  && git status --short
```

Expected: the grep prints `0`; `test result: ok` with 187 tests; `git status --short` shows exactly `?? .idea/`, `?? docs/`, `?? sql/` and `M src/params.rs`.

- [ ] **Step 9: Branch and commit**

This repo is on `main`. Branch first.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape \
  && git checkout -b feat/params-ranking \
  && git add src/params.rs \
  && git commit -m "$(cat <<'EOF'
feat(params): rank exact and prefix matches above substrings

The lookup filtered keys by substring and took the first ten in file
order, which is alphabetical, so a query returned an arbitrary slice
rather than the closest matches. +params smithing cannonball buried
Steel_cannonball at rank 16 behind the chainshot and incendiary
variants, and +params hitpoints goblin answered with
Angry_goblin_(lvl_47).

Matches are now ranked exact, then prefix, then substring, tied by
fewest extra underscore-separated tokens and then alphabetically. The
token count lifts a buried canonical answer above its longer variants.
The alphabetical tiebreak keeps a list of peers in the order it already
had -- an earlier attempt tied by key length instead and turned the
thirty-one teleports into a jumble.

No value changes and no match disappears; only the order within the
matched set does. Ranking lives in a pure function so it can be tested
without loading the database, which is why the existing tests in this
module return before the file is read.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)" && git show --stat HEAD | tail -3
```

Expected: `1 file changed`, `src/params.rs`.

---

### Task 2: Crafting and Magic parsers

**Files:**
- Modify: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/parse.py`
- Test: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/test_parse.py`

**Interfaces:**
- Consumes: `parse.extract_template`, `parse._all_template_blocks`, `parse._skill_indices`, `parse.parse_number`, `parse._format_xp`, `parse._key`, `parse._xp_field`, `parse.UNPARSEABLE` — all existing in the same module.
- Produces:
  - `recipe_entries(wikitext: str, skill: str) -> list[tuple[str, str]]`
  - `crafting_entries(wikitext: str) -> list[tuple[str, str]]` — `recipe_entries(wikitext, "Crafting")`
  - `magic_entries(wikitext: str) -> list[tuple[str, str]]`
  - `smithing_entries` keeps its existing signature and delegates to `recipe_entries(wikitext, "Smithing")`.

- [ ] **Step 1: Write the failing tests**

Add to `test_parse.py`:

```python
RECIPE_CRAFTING = """{{Recipe
|skill1 = Crafting
|skill1lvl = 20
|skill1exp = 40
|output1 = Emerald ring
}}
"""

RECIPE_CRAFTING_SECONDARY = """{{Recipe
|skill1 = Smithing
|skill1exp = 30
|skill2 = Crafting
|skill2exp = 55
|output1 = Gold bracelet
}}
"""

RECIPE_ANCHORED_OUTPUT = """{{Recipe
|skill1 = Crafting
|skill1exp = 4
|output1 = Fishing Crane#Repaired
}}
"""

SPELL_PLAIN = """{{Infobox Spell
|name = Telekinetic Grab
|level = 33
|spellbook = Normal
|exp = 43
}}
"""

SPELL_FORMULA = """{{Infobox Spell
|name = Alchemic Divergence
|level = 88
|spellbook = Arceuus
|exp = 10 * items
}}
"""


def test_recipe_entries_reads_the_named_skill():
    assert parse.recipe_entries(RECIPE_CRAFTING, "Crafting") == [
        ("Emerald_ring", "40")
    ]


def test_recipe_entries_finds_the_skill_in_any_slot():
    assert parse.recipe_entries(RECIPE_CRAFTING_SECONDARY, "Crafting") == [
        ("Gold_bracelet", "55")
    ]


def test_recipe_entries_ignores_other_skills():
    assert parse.recipe_entries(RECIPE_CRAFTING, "Smithing") == []


def test_smithing_entries_still_works_via_the_generalised_helper():
    block = (
        "{{Recipe\n|skill1 = Smithing\n|skill1exp = 375\n"
        "|output1 = Rune platebody\n}}\n"
    )
    assert parse.smithing_entries(block) == [("Rune_platebody", "375")]


def test_crafting_entries_is_recipe_entries_for_crafting():
    assert parse.crafting_entries(RECIPE_CRAFTING) == [("Emerald_ring", "40")]


def test_recipe_entries_strips_a_page_anchor_from_the_output_name():
    # "Fishing Crane#Repaired" must not ship a '#' in a key.
    assert parse.recipe_entries(RECIPE_ANCHORED_OUTPUT, "Crafting") == [
        ("Fishing_Crane", "4")
    ]


def test_magic_entries_reads_a_spell():
    assert parse.magic_entries(SPELL_PLAIN) == [("Telekinetic_Grab", "43")]


def test_magic_entries_records_a_formula_rather_than_guessing():
    parse.UNPARSEABLE.clear()
    assert parse.magic_entries(SPELL_FORMULA) == []
    assert parse.UNPARSEABLE == [
        "Alchemic Divergence: exp='10 * items'"
    ], parse.UNPARSEABLE


def test_magic_entries_returns_empty_without_template():
    assert parse.magic_entries("Just prose.\n") == []
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest test_parse.py -v`
Expected: FAIL — `AttributeError: module 'parse' has no attribute 'recipe_entries'`.

- [ ] **Step 3: Generalise the recipe parser and add the spell parser**

In `parse.py`, replace the body of `smithing_entries` with a delegation and add the new functions:

```python
def recipe_entries(wikitext: str, skill: str) -> list[tuple[str, str]]:
    """(key, value) for every {{Recipe}} block whose skill matches `skill`.

    Keyed by the block's output, valued by the experience of the skill slot
    that names `skill` -- which is not always slot 1.
    """
    wanted = skill.strip().lower()
    out: list[tuple[str, str]] = []
    for block in _all_template_blocks(wikitext, "Recipe"):
        fields = extract_template(block, "Recipe")
        if not fields:
            continue
        index = None
        for n in _skill_indices(fields):
            if fields.get(f"skill{n}", "").strip().lower() == wanted:
                index = n
                break
        if index is None:
            continue
        name = fields.get("output1", "").strip()
        if not name:
            continue
        # A page anchor is not part of the item name and must never reach a key.
        name = name.split("#", 1)[0].strip()
        if not name:
            continue
        value = _xp_field(fields, f"skill{index}exp", name)
        if value is None:
            continue
        out.append((_key(name), value))
    return out


def smithing_entries(wikitext: str) -> list[tuple[str, str]]:
    """(key, value) for every Smithing {{Recipe}} block on the page."""
    return recipe_entries(wikitext, "Smithing")


def crafting_entries(wikitext: str) -> list[tuple[str, str]]:
    """(key, value) for every Crafting {{Recipe}} block on the page."""
    return recipe_entries(wikitext, "Crafting")


def magic_entries(wikitext: str) -> list[tuple[str, str]]:
    """(key, value) from {{Infobox Spell}}.

    Two spells state a formula rather than a number; those are recorded in
    UNPARSEABLE for a human rather than guessed at.
    """
    fields = extract_template(wikitext, "Infobox Spell")
    if not fields:
        return []
    name = fields.get("name", "").strip()
    if not name:
        return []
    value = _xp_field(fields, "exp", name)
    if value is None:
        return []
    return [(_key(name), value)]
```

Delete the old `smithing_entries` body — the one that inlines the block loop — so only the delegation remains.

- [ ] **Step 4: Run the full suite**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -m pytest -v`
Expected: PASS, 75 passed (66 existing plus your 9). Every pre-existing Smithing test must still pass — they now exercise the generalised path.

- [ ] **Step 5: Verify against real fetched pages**

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import fetch, parse
for t in ['Emerald ring', 'Green dragonhide body', 'Molten glass', 'Rune platebody']:
    print(f'{t:24} crafting={parse.crafting_entries(fetch.fetch_raw(t))} smithing={parse.smithing_entries(fetch.fetch_raw(t))}')
for t in ['Telekinetic Grab', 'High Level Alchemy', 'Blood Barrage', 'Alchemic Divergence']:
    print(f'{t:24} magic={parse.magic_entries(fetch.fetch_raw(t))}')
print('UNPARSEABLE:', parse.UNPARSEABLE)
"
```

Expected: `Rune platebody` yields a Smithing entry and no Crafting one; `Telekinetic Grab -> [('Telekinetic_Grab', '43')]`; `High Level Alchemy -> [('High_Level_Alchemy', '65')]`; `Blood Barrage -> [('Blood_Barrage', '51')]`; `Alchemic Divergence -> []` with its formula recorded. Report what each prints.

**No commit.** Throwaway tool.

---

### Task 3: Generate the two reconcile reports

**Files:**
- Modify: `/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/report.py`
- Create (output): `report-crafting.txt`, `report-magic.txt`, `wiki-crafting-keys.json`, `wiki-magic-keys.json` in the same directory
- Read only: `rust-reinze/lib/Database.ini`

**Interfaces:**
- Consumes: `parse.crafting_entries`, `parse.magic_entries` (Task 2), plus the existing `report._gather` and `report.run_section`.
- Produces: the four output files, read by Tasks 4 and 5.

No unit test — this is an integration run whose reports are the deliverable. Its gate is the Step 3 sanity checks. Do not modify the 75 tests, which must still pass.

- [ ] **Step 1: Point `main()` at the two new sections**

`report.py` already has `_gather` and `run_section` from the previous run. Replace the three `run_section(...)` calls in `main()` with:

```python
    ini_text = INI_PATH.read_text(encoding="utf-8")
    run_section("Crafting", "Template:Recipe", parse.crafting_entries, ini_text)
    run_section("Magic", "Template:Infobox Spell", parse.magic_entries, ini_text)
```

Leave the Smithing, Farming and Hunter calls out — those sections are committed and re-running the 3,724-page Recipe sweep for them wastes time. `run_section` already snapshots and slices `parse.UNPARSEABLE` per section, writes `report-<slug>.txt`, and writes `wiki-<slug>-keys.json`; it needs no changes.

- [ ] **Step 2: Run it**

Run: `cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u report.py 2>report.stderr.log`

The Crafting sweep walks all 3,724 `{{Recipe}}` pages, but they are already cached from the Smithing run, so this is fast. Magic fetches 226 pages, most of them new.

**Do not use `pgrep -f 'python3 -u report.py'` to check whether it is still running** — that pattern matches its own wrapper command line and reports "still running" forever. Use `ps -eo pid,cmd | grep -E '[p]ython3 -u report\.py'`.

- [ ] **Step 3: Sanity-check the output**

```bash
cd /home/rohara/.claude/jobs/5422e5c0/tmp/wikitool && python3 -u -c "
import json, pathlib, re
for s in ('crafting','magic'):
    t = pathlib.Path(f'report-{s}.txt').read_text()
    keys = json.loads(pathlib.Path(f'wiki-{s}-keys.json').read_text())
    unch = re.search(r'unchanged: (\d+)', t).group(1)
    counts = re.findall(r'--- ([A-Z][A-Za-z ,/-]*?) \((\d+)\)', t)
    print(s, '| wiki keys:', len(keys), '| unchanged:', unch, '|',
          ' '.join(f'{k.strip()}={v}' for k, v in counts))
"
```

Expected and required:
- Crafting wiki keys around 500 (measured: 508 distinct keys before ambiguity withholding), with roughly 9 ambiguous.
- Magic wiki keys around 224 (226 pages, 2 of which state a formula).
- For each section, `unchanged + changed + wiki_only` must equal its wiki key count. If not, ambiguous keys are leaking into the data — stop and fix.
- Magic UNPARSEABLE should contain exactly the two `10 * items` formulae.
- Crafting UNPARSEABLE should contain `Guardian essence: skill1exp='1-5'` and the `Fishing Crane` formula entries.
- Report every bucket size for both sections.

- [ ] **Step 4: Read both reports in full**

Do not proceed to Task 4 without having read `report-crafting.txt` and `report-magic.txt`. Tasks 4 and 5 are hand edits driven entirely by their contents.

**No commit.** Throwaway.

---

### Task 4: Rewrite the `[Crafting]` section

**Files:**
- Modify: `rust-reinze/lib/Database.ini` — the `[Crafting]` section only
- Read: `report-crafting.txt`, `wiki-crafting-keys.json`

**Interfaces:**
- Consumes: Task 3's Crafting report.
- Produces: nothing. Task 5 is independent.

**Rules:**

- One alphabetically sorted block. Keys are the wiki's exact item name with spaces replaced by `_`. Values are bare numbers.
- `Yak-hide_legs` and `Yak-hide_body` keep their hyphens — those are genuine wiki names. Verify against their pages.
- **The 9 ambiguous keys** are all shapes already ruled on in the Smithing audit. Resolve each against its own page and say which shape it fell into:
  - `Purging_staff` 730 vs 73 — first-time versus subsequent. Take the **first-time** value, as `Emberlight` and `Chromium_ingot` did.
  - `Molten_glass` 20/130/180 and `Bow_string` 15/75 — alternate methods. Take the **standard** method.
  - `Blade_of_saeldor_(c)`, `Bow_of_faerdhinen_(c)`, `Crystal_felling_axe`, `Salve_amulet` — zero versus real. Take the **non-zero**.
  - `Armadylean_plate` 210/840/630 and `Crushed_gem` 3.7/5/6.3 — tiers, with no prior ruling. Decide each individually against its page and explain your choice.
- **Both screens on every addition**, sanity-checked against a known positive first (`Monodon bones` for beta, `Smoker canister` for gone).
- Zero-XP rows excluded.
- `Guardian_essence` has `skill1exp = '1-5'`, a range in a bare-number section. Decide by hand and say what you chose and why. Do not invent a midpoint.
- Every INI-ONLY entry decided individually — keep, rename, correct, or drop. Drop ONLY as duplicate, superseded, or demonstrably not Old School.
- Note that `Spinning_Wool=2.5`/`Ball_of_wool=2.5` and `Flax=15`/`Bow_string=15` are process-versus-product pairs already in the INI. Resolve them to the wiki's item names rather than keeping both spellings of the same action.

- [ ] **Step 1: Print your disposition for every INI-ONLY entry**

keep / rename to X / correct to N / drop because Y, one line each. This is the core of the task.

- [ ] **Step 2: Resolve the 9 ambiguities against their own pages**

Print the resolution and the shape for each.

- [ ] **Step 3: Spot-check ten additions and run both screens**

Pick ten of the wiki-only additions spanning different families (gems, leather, jewellery, glass, battlestaves) and confirm each against its own page. Report what the screens rejected.

- [ ] **Step 4: Apply the edit**

Use `Edit` on `rust-reinze/lib/Database.ini`, replacing the whole `[Crafting]` section.

- [ ] **Step 5: Run the mechanical scan**

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && python3 - <<'EOF'
import json, pathlib, re
raw = open('lib/Database.ini', newline='').read()
lines = raw.replace('\r\n', '\n').split('\n')
i = lines.index('[Crafting]')
rows = []
for l in lines[i+1:]:
    if l.startswith('['): break
    if l.strip(): rows.append(l)
keys = [r.split('=', 1)[0] for r in rows]
print('entries:', len(rows))
assert not [r for r in rows if r.endswith('=')], 'empty value'
assert not [k for k in keys if ' ' in k], 'space in key'
assert not [k for k in keys if re.search(r'<|\[\[|\]\]|&#|#', k)], 'markup or anchor in key'
bad = [r for r in rows if not re.fullmatch(r'\d+(\.\d+)?', r.split('=', 1)[1])]
assert not bad, 'value not a bare number: %s' % bad[:5]
assert not [r for r in rows if r.split('=', 1)[1] == '0'], 'zero-XP row'
low = [k.lower() for k in keys]
assert len(low) == len(set(low)), 'duplicate key'
assert keys == sorted(keys, key=str.lower), 'not sorted'
wiki = set(json.loads(pathlib.Path(
    '/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/wiki-crafting-keys.json').read_text()))
KEPT = {
    # Hand-kept entries with no qualifying Recipe block. Fill in from your
    # Step 1 and Step 2 dispositions, one comment per entry naming the page
    # you verified it against.
}
stray = [k for k in keys if k not in wiki and k not in KEPT]
assert not stray, 'keys neither wiki names nor justified: %s' % stray
print('CRLF:', raw.count('\r\n'), 'of', len(lines) - 1)
print('Crafting section clean:', len(rows), 'entries')
EOF
```

- [ ] **Step 6: Validate with the real `ini` crate**

Create `reinze-lib-runescape/examples/ini_smoke.rs`:

```rust
// Throwaway INI validator. Created, run, and deleted within one task.
use ini::Ini;

fn main() {
    let path = std::env::args().nth(1).expect("usage: ini_smoke <path>");
    let ini = Ini::load_from_file(&path).expect("Ini::load_from_file failed");
    for section in ["Crafting", "Magic", "Smithing", "Farming", "Hunter", "Prayer", "Construction"] {
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

Expected: a line per section, `INI OK`, then `test result: ok` with 187 passing.

- [ ] **Step 7: Delete the example and confirm the plugin repo is clean**

```bash
rm /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples/ini_smoke.rs
rmdir /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples 2>/dev/null
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && git status --short
```

Expected: exactly `?? .idea/`, `?? docs/`, `?? sql/` — Task 1's change is already committed on `feat/params-ranking`.

- [ ] **Step 8: Confirm nothing else moved**

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze \
  && git diff HEAD -- lib/Database.ini | grep -E '^[-+]\[[A-Za-z]+\]' || echo "no section header changed"
```

- [ ] **Step 9: Commit**

Body paragraphs wrapped at 72 columns: `Crafting (118 -> N).` and what was wrong; what was added, grouped rather than enumerated; how the 9 ambiguities were resolved; the `Guardian essence` decision; what was dropped and why; what was left alone. Read `git show e694896` first and match its voice.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && git add lib/Database.ini && git commit -m "$(cat <<'EOF'
chore(data): Update Crafting params from wiki

Crafting (118 -> N). ...

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)" && git show --stat HEAD | tail -3
```

Expected: `1 file changed`, `lib/Database.ini`.

---

### Task 5: Rewrite the `[Magic]` section

**Files:**
- Modify: `rust-reinze/lib/Database.ini` — the `[Magic]` section only
- Read: `report-magic.txt`, `wiki-magic-keys.json`

**Interfaces:**
- Consumes: Task 3's Magic report.
- Produces: nothing. This is the last task.

**Rules:**

- One alphabetically sorted block. Keys are the wiki's exact spell name with spaces replaced by `_`. Values are bare numbers.
- **`Alchemic Divergence` and `Alchemic Convergence` state `exp = '10 * items'`** — a formula with no fixed value, like Hunter's Moss Lizard. Omit them or carry a defensible number, and say which and why. Do not invent one the page does not support.
- The section spans four spellbooks — Normal 82, Ancient 25, Lunar 44, Arceuus 70, plus three marked "all". All are in scope; the section does not distinguish them today and should not start.
- **Both screens on every addition**, sanity-checked against a known positive first.
- Zero-XP rows excluded. Some utility spells legitimately award no Magic XP; those are noise in an XP lookup.
- Every INI-ONLY entry decided individually. Drop ONLY as duplicate, superseded, or demonstrably not Old School.
- `Telekinetic_Grab=43` was repaired in commit `24a15d9` from a corrupted key. It is correct; leave its value alone.

- [ ] **Step 1: Print your disposition for every INI-ONLY entry**

keep / rename to X / correct to N / drop because Y, one line each.

- [ ] **Step 2: Decide the two formula spells**

State your choice and the evidence from their pages.

- [ ] **Step 3: Spot-check ten values and run both screens**

Pick ten spanning all four spellbooks, confirm each against its own page's `exp` field. Report what the screens rejected.

- [ ] **Step 4: Apply the edit**

Use `Edit` on `rust-reinze/lib/Database.ini`, replacing the whole `[Magic]` section.

- [ ] **Step 5: Run the mechanical scan**

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && python3 - <<'EOF'
import json, pathlib, re
raw = open('lib/Database.ini', newline='').read()
lines = raw.replace('\r\n', '\n').split('\n')
i = lines.index('[Magic]')
rows = []
for l in lines[i+1:]:
    if l.startswith('['): break
    if l.strip(): rows.append(l)
keys = [r.split('=', 1)[0] for r in rows]
print('entries:', len(rows))
assert not [r for r in rows if r.endswith('=')], 'empty value'
assert not [k for k in keys if ' ' in k], 'space in key'
assert not [k for k in keys if re.search(r'<|\[\[|\]\]|&#|#', k)], 'markup or anchor in key'
bad = [r for r in rows if not re.fullmatch(r'\d+(\.\d+)?', r.split('=', 1)[1])]
assert not bad, 'value not a bare number: %s' % bad[:5]
assert not [r for r in rows if r.split('=', 1)[1] == '0'], 'zero-XP row'
low = [k.lower() for k in keys]
assert len(low) == len(set(low)), 'duplicate key'
assert keys == sorted(keys, key=str.lower), 'not sorted'
assert 'Telekinetic_Grab' in keys, 'the repaired key was lost'
wiki = set(json.loads(pathlib.Path(
    '/home/rohara/.claude/jobs/5422e5c0/tmp/wikitool/wiki-magic-keys.json').read_text()))
KEPT = {
    # Hand-kept entries with no Infobox Spell page. Fill in from your Step 1
    # dispositions, one comment per entry naming the page you checked.
}
stray = [k for k in keys if k not in wiki and k not in KEPT]
assert not stray, 'keys neither wiki names nor justified: %s' % stray
print('CRLF:', raw.count('\r\n'), 'of', len(lines) - 1)
print('Magic section clean:', len(rows), 'entries')
EOF
```

- [ ] **Step 6: Validate with the real `ini` crate**

Create `reinze-lib-runescape/examples/ini_smoke.rs` with exactly this content:

```rust
// Throwaway INI validator. Created, run, and deleted within one task.
use ini::Ini;

fn main() {
    let path = std::env::args().nth(1).expect("usage: ini_smoke <path>");
    let ini = Ini::load_from_file(&path).expect("Ini::load_from_file failed");
    for section in ["Crafting", "Magic", "Smithing", "Farming", "Hunter", "Prayer", "Construction"] {
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

- [ ] **Step 7: Delete the example and confirm the plugin repo is clean**

```bash
rm /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples/ini_smoke.rs
rmdir /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape/examples 2>/dev/null
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape && git status --short
```

Expected: exactly `?? .idea/`, `?? docs/`, `?? sql/`.

- [ ] **Step 8: Confirm only Magic moved**

The Crafting commit is already `HEAD`, so diffing against `HEAD` shows this task alone:

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze \
  && git diff HEAD -- lib/Database.ini | grep -E '^[-+]\[[A-Za-z]+\]' || echo "no section header changed"
```

- [ ] **Step 9: Commit**

Body paragraphs wrapped at 72 columns: `Magic (160 -> N).` and what was wrong; what was added, grouped by spellbook; how the two formula spells were decided; what was dropped and why; what was left alone. Match the voice of `git show e694896`.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze && git add lib/Database.ini && git commit -m "$(cat <<'EOF'
chore(data): Update Magic params from wiki

Magic (160 -> N). ...

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)" && git show --stat HEAD | tail -3
```

Expected: `1 file changed`, `lib/Database.ini`.

---

## Final verification

Run every command and paste the output.

```bash
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/rust-reinze \
  && git log --oneline -4 && git status --short
cd /home/rohara/.agent-deck/multi-repo-worktrees/788b004e/reinze-lib-runescape \
  && git log --oneline -2 && git status --short && cargo test --quiet 2>&1 | tail -5
```

- [ ] `rust-reinze` shows the two `chore(data):` commits on top of `d2dd619`, one file each.
- [ ] `reinze-lib-runescape` is on `feat/params-ranking` with one `feat(params):` commit touching only `src/params.rs`.
- [ ] `cargo test` in `reinze-lib-runescape` passes with 187 tests.
- [ ] `rust-reinze` `git status --short` shows only the pre-existing untracked entries: `.idea/`, `conf/rizon.toml.example` and the March docs.
- [ ] `reinze-lib-runescape` `git status --short` shows only `.idea/`, `docs/`, `sql/` — no `examples/`.
- [ ] Nothing from `/home/rohara/.claude/jobs/5422e5c0/tmp/` is committed to either repo.
- [ ] The mechanical scans from Tasks 4 and 5 both still pass against the final file.
- [ ] `[Prayer]` 73, `[Construction]` 483, `[Smithing]` 358, `[Farming]` 94, `[Hunter]` 86 are byte-identical to `d2dd619`, and the six combat sections plus `[Slayer]` are untouched.
