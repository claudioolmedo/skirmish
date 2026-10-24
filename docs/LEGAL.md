# Skirmish II — Legal Foundation

*Last updated: 2026-10-23*

Skirmish II is built to last — "a chess for 200 years". Chess outlived
its makers because nobody owns its rules and its form is free for anyone
to implement. This document states, in plain language, how Skirmish II
is put together so that the same is true here: what the project owns,
what it borrows (facts only), what it never touches, and the terms under
which others may build on it.

This is a statement of the project's method and provenance. It is not
legal advice and it makes no claims against anyone.

## 1. The license

All original code and art in this repository are the work of the
project's contributors and are licensed under the
[Apache License 2.0](../LICENSE) (see the `LICENSE` file at the
repository root; copyright 2026 Claudio Olmedo).

Apache-2.0 was chosen deliberately:

- It is **permanent and irrevocable** for code already released under
  it — the game can outlive any company, maintainer, or platform.
- It includes an **express patent grant** from contributors.
- It expressly **does not grant trademark rights**: the license covers
  the code and the art, **not** the name "Skirmish II", which remains
  the project owner's.

## 2. The clean-room method

Skirmish II is a clean-room implementation of a real-time strategy game:

- Every line of code, every sprite, every sound, every name, and every
  UI layout is created from scratch for this project.
- The commercial game that inspired it was **never** decompiled,
  reverse-engineered, or data-mined. Its binary, assets, and extracted
  data are never used — no exceptions.
- Behavior targets come only from what is publicly observable and
  publicly documented: published guides, public videos of gameplay,
  and the community wiki.

## 3. Sources policy

- **Facts are free; expression is not copied.** Game mechanics, rules,
  and numeric values (unit statistics, costs, timings, rates) are facts
  and ideas. The project records those facts from public documentation
  and expresses them in its own code, its own words, and its own art.
- **No prose is copied from any source.** The community wiki, for
  example, is published under CC BY-SA; out of respect for that license
  and for clarity of provenance, this project never copies its text (or
  any other source's text) into the game, the site, or these documents.
  Values and behavior are recorded in the project's own words.
- **Citations are kept.** The study dossiers and the mechanics rulebook
  record the public source behind each documented behavior, so any
  value in the game can be traced to the public documentation it came
  from.

## 4. Asset provenance

- **Art:** all art is original — drawn for this project, or produced as
  AI-assisted original drawings through the project's own sprite
  pipeline (`tools/sprite-pipeline/`, `docs/SPRITE-PIPELINE.md`). No
  sprite, texture, icon, font, or image from any existing game is used
  anywhere.
- **Audio:** every sound is synthesized in code (Web Audio) at runtime.
  There are no audio files, and no music or sound effect from any
  existing game is used.
- **Names and text:** all names, interface text, and translations in the
  game are the project's own writing. Where a generic historical or
  functional term coincides with a term another game also uses (for
  example "Town Center" or "Villager"), it is used as the plain
  descriptive name of the thing it names.

## 5. Trademark posture

- The project's name is **Skirmish II**. "Skirmish" is an ordinary
  English word for a small fight; the name is the project's own.
- Skirmish II is an independent community project. It is **not
  affiliated with, endorsed by, or sponsored by Microsoft, Ensemble
  Studios, or Xbox Game Studios**.
- Third-party trademarks are referenced **only descriptively** — to say
  what inspired the game and what its documented behavior targets are.
  *Age of Empires* is a trademark of Microsoft. No third-party logo,
  box art, or trade dress is used anywhere in the game or on the site.
- The Apache-2.0 license of the code and art grants **no rights to the
  "Skirmish II" name**. Forks may use the code and art under the
  license; they may not present themselves as Skirmish II.

## 6. Contributor terms

- **Inbound = outbound.** By submitting a contribution (code, art,
  text, or data) to this project, the contributor licenses it to the
  project and to the public under the Apache License 2.0, the same
  license the project ships under.
- **The chess gate.** The mechanics are treated as finished, like the
  rules of chess. Contributions may implement or correct a mechanic
  only to converge on publicly documented behavior, cited in the pull
  request. Original rule inventions presented as fidelity, rebalances,
  and rule changes are out of scope and are closed.
- Contributors warrant, by the act of submitting under this policy,
  that their contribution is their own work or is properly licensed for
  them to submit — in particular, that it contains no code, art, text,
  or data copied from any existing game (see the pull request
  checklist).

## 7. Privacy posture

- The game has **no accounts and no sign-up**, and collects **no
  personal data**. Single-player runs entirely on the player's device.
- Online play stores only anonymous match counters. End-of-match
  feedback is optional and anonymous.
- The full statement is the site's privacy policy:
  <https://skirmish.online/privacy/>.

## 8. Advertising posture

- The game is community-funded by design (see
  [SUSTAINABILITY.md](SUSTAINABILITY.md)): if advertising is enabled on
  the site, it appears **only** in clearly labeled slots on the site's
  pages, never inside the simulation, and it is covered by the privacy
  policy linked above. Advertising configuration lives in the open, in
  `web-preview/ads.js`.

## 9. Contact

Rights-holders and anyone else with a concern about anything in this
project: please open an issue on the public repository —
<https://github.com/claudioolmedo/skirmish/issues> — and it will be
read and answered. The project would always rather fix a problem than
argue about one.
