# Status

## Rules engine (`rules-core`)

Implemented and covered by tests:

- Commander setup (903.6–903.7): command zone, 40 life, seven cards, London mulligan with the free multiplayer mulligan (103.5c), random or chosen starting player, no first-turn draw skip in multiplayer (103.8c).
- Turn structure (500–514) including cleanup discard and the 514.3a extra-priority case.
- Priority (117): passing in succession, priority returning to the actor, SBAs and triggers before every priority grant (117.5).
- Casting (601) with timing rules, targets, X, commander tax (903.8), automatic or manual mana payment (hybrid, Phyrexian, colorless), mana abilities (605).
- Stack and resolution (405, 608): LIFO, target re-check and "doesn't resolve" (608.2b), countering.
- Triggered abilities (603): ETB, dies (with look-back, 603.10a, including simultaneous deaths), attacks, combat damage to a player, upkeep/end step, cast triggers. APNAP stacking and per-player ordering and targeting.
- State-based actions (704.5a–d, f–j, m, q) plus commander damage (903.10a) and commanders returning to the command zone (903.9a).
- Commander replacement to the command zone from hand/library moves (903.9b), as a standing per-player choice.
- Combat (506–511): attacking multiple players, blocking restrictions (flying/reach, menace), first strike and double strike steps, trample, deathtouch, lifelink, commander damage tracking.
- Layer system (613): all layers and 7a–7d sublayers, CDAs, timestamps, dependencies (Blood Moon/Urborg), 613.6 continuation (Humility/Opalescence both orders), locked-in vs. dynamic affected sets (611.2c/611.3a), control-changing effects.
- Players leaving a multiplayer game (800.4a, 800.4j).

Known gaps (planned):

- Choices during resolution (choose a card to discard/sacrifice, "you may", choose a color): currently auto-chosen.
- Legend rule keeps the newest copy instead of asking.
- Damage assignment among multiple blockers is automatic (lethal in order) instead of a player choice (510.1c).
- Replacement/prevention effects framework (614–616) beyond the commander replacement.
- Planeswalker/battle attacks, protection, ward, copy/face-down (layer 1) beyond basic copy, text-changing (layer 3).
- Choices made during resolution (choose a mode, a card, a pile) and manual combat damage assignment.

## Card data (`carddb`)

- MTGJSON `AtomicCards` import (multi-face cards, color identity, commander legality, leadership skills).
- Update check against MTGJSON `Meta.json` (download happens in the app).
- Oracle-text parser for keywords, mana abilities, common triggers, anthems and common spell effects.
- Decklist import (Moxfield/Archidekt/MTGO text, `*CMDR*`) and validation: size, singleton, color identity, commander eligibility, partner/background pairs, banned list, unsupported-card report.

## Client (`client-api`, `engine-wasm`, `app/`)

- `client-api` turns a game into a per-player view with hidden information removed (only your own hand and nobody's library), plus hints for what you can legally do right now. Practice opponents play by simple rules.
- The engine compiles to WebAssembly and runs inside the desktop window (and in a browser for development).
- Table UI: opponents across the top with focus switching, your board split into lands and creatures, fanned hand, command zone, stack panel, card preview on hover, targeting, attacking and blocking, auto-pass when you have nothing to do (turn off with "Full control").
- Desktop shell (Tauri 2) shipped as a portable folder. On launch it checks this repo's latest GitHub release, downloads the `*-windows-portable.zip`, verifies its SHA-256, swaps the files in place and restarts. Files under `data/` are never overwritten.
- Every merge to `main` publishes a release `v0.1.<run>` via `.github/workflows/release.yml`.

- Card data: on launch the desktop app checks MTGJSON's `Meta.json` and, when there are new cards, downloads `AtomicCards.json.gz` into `data/cards/` (checked against MTGJSON's SHA-256, parsed before it replaces the old file). Built-in starter cards fill in until the first download.
- Decks: paste a Moxfield/Archidekt/MTGO list; it's checked live against the card pool (commander, size, singleton, color identity, banned list, unknown and unsupported cards). Saved as `data/decks/*.json`. Decks that break deck-building rules can still be played casually; decks with unknown cards or no commander can't.
- Game setup: your deck, 1 to 3 computer opponents, each with a deck (or a random built-in one) and a play style.

## Computer players (`client-api/src/ai`)

- For each option (land, spell with each sensible target and X, ability, attack plan, block plan, trigger targets) the AI copies the game, plays the option forward with the real engine (stack resolves, combat finishes, other players' blocks guessed from the board), and scores the result.
- The score weighs its own life (low life matters more), board, cards in hand and commander damage against each opponent's, weighted by how threatening they look and how much they've attacked or targeted it lately, minus the risk of the crack-back when it taps out.
- Plays like a person: creatures after combat, instants held for the opponents' end step or combat, counterspells only when something on the stack is worth it, chump blocks only when the damage really matters, near-equal options picked by taste.
- Three styles (Aggressive, Balanced, Controlling) change risk-taking, patience and how much it values cards.
- It never looks at hidden cards: only the battlefield, life totals, card counts and its own hand.
- Thinking time: each decision comes with a human-like delay (longer for big spells and attacks, instant for empty steps); "Fast opponents" shortens it.
- Tests: wins 12 of 12 against the old simple bot; takes lethal, avoids bad attacks, chump-blocks only at low life, bolts the biggest threat at end of turn, holds removal when nothing's worth killing.

### Next

- Steam lobbies and invites (host-authoritative: the host runs the engine, players send actions).
- More card scripting coverage (see the unsupported list in the deck screen).
