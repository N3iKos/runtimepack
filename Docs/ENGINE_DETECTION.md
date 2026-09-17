# Engine Detection Specification

## 1. Principle

Detection is static, deterministic, non-executing, and evidence-based.

The detector scans filenames, selected small text/config files, and safe metadata. It does not run executables or game scripts.

## 2. Candidate scoring

Each detector produces:

```json
{
  "engine": "rpgmaker-vxace",
  "confidence": "EXACT",
  "score": 220,
  "evidence": [],
  "conflicts": []
}
```

Suggested thresholds:

```text
>= 200 EXACT
>= 140 HIGH
>= 80  MEDIUM
>= 30  LOW
< 30   NONE
```

Weights are internal policy and may evolve, but evidence rules must stay explicit.

## 3. RPG Maker XP

JoiPlay's public RGA template shows XP with `Game.ini`, library `RGSS104E.dll`, and `Scripts=Data\Scripts.rxdata` [S20].

Evidence:

```text
+120 Data/Scripts.rxdata exists
+80  Game.ini exists and Scripts ends in .rxdata
+30  Game.ini Library contains RGSS1
+10  Game.exe exists
```

Required for automatic HIGH or EXACT:

- `Game.ini`; and
- `.rxdata` script evidence.

Do not classify merely because `Game.exe` exists.

## 4. RPG Maker VX

JoiPlay's template uses `Scripts=Data\Scripts.rvdata` and an RGSS2 library [S21].

```text
+120 Data/Scripts.rvdata
+80  Game.ini Scripts=.rvdata
+30  Game.ini Library contains RGSS2
+10  Game.exe
```

## 5. RPG Maker VX Ace

JoiPlay's template uses `Scripts=Data\Scripts.rvdata2` and `RGSS301.dll` [S22].

```text
+120 Data/Scripts.rvdata2
+80  Game.ini Scripts=.rvdata2
+30  Game.ini Library contains RGSS3
+10  Game.exe
```

## 6. RPG Maker MV

High-confidence markers:

```text
index.html
js/rpg_core.js
js/rpg_managers.js
data/System.json
```

Accept either root layout or a deployed nested `www/` layout, but normalize logical root before final classification.

Official MV documentation confirms its HTML5/JavaScript web execution model [S13].

Conflict rule:

- if `rmmz_core.js` exists alongside `rpg_core.js`, return AMBIGUOUS unless a known deployment migration rule proves one is stale.

## 7. RPG Maker MZ

High-confidence markers:

```text
index.html
js/rmmz_core.js
js/rmmz_managers.js
data/System.json
```

Official MZ documentation confirms web browser deployment [S14].

## 8. Ren'Py

Possible markers:

```text
game/
*.rpy
*.rpyc
*.rpa
renpy/
```

Detection must distinguish:

- source distribution;
- desktop packaged distribution;
- already-Android package (not accepted as source in v1).

Version extraction is a separate phase and may fail even if engine detection succeeds.

Detection result may therefore be:

```text
engine = RenPy
confidence = HIGH
version = UNKNOWN
```

Build then fails with `RNP-110 RUNTIME_VERSION_UNRESOLVED` unless policy resolves it safely.

## 9. Tyrano

Official TyranoScript source layout includes `index.html`, `tyrano/`, `data/`, and KAG engine files [S10/S12]. JoiPlay's RGA template uses `index.html` as Tyrano entrypoint [S23].

Evidence:

```text
+80 index.html
+80 tyrano/tyrano.js
+50 tyrano/plugins/kag/
+40 data/
+30 data/scenario/
```

`index.html` alone is generic HTML and never enough.

## 10. Ambiguous HTML games

If a source contains `index.html` but lacks an MV/MZ/Tyrano signature:

```text
engine = UNKNOWN
reason = DET-150_GENERIC_HTML_OUT_OF_SCOPE
```

Generic HTML is not in v1 scope even though JoiPlay supports it.

## 11. Security limits

Detector must limit:

- maximum bytes read from text files;
- recursive depth;
- file count before warning;
- path length;
- archive expansion ratio if archive input is enabled.

No parser may trust file extensions alone.

## 12. Detection report

Every detection output records:

```json
{
  "engine": "rpgmaker-vxace",
  "confidence": "EXACT",
  "evidence": [
    {"rule":"DET-RGSS-VA-001","path":"Game.ini"},
    {"rule":"DET-RGSS-VA-002","path":"Data/Scripts.rvdata2"}
  ],
  "negative_evidence": [],
  "conflicts": []
}
```

The `explain` command prints this report in human-readable form.
