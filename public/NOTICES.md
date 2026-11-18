# Notices

## Original game content

- **Title:** Need for Madness (Java applet).
- **Author / publisher as recovered from the game itself:** the credits screen
  reads `"At Radicalplay.com"`, `"By Omar Waly"` and
  `"For any comments: Omar@radicalplay.com"` (`xtGraphics.java`, lines
  1483–1493). It also credits game testers `"Khaled Helmy, Ismail Gorilaz,
  Karim AboSamra, Mahmoud Waly, Karim Khadem, Ahmed Ismail and Mahmoud
  EzzElDien (Turbo)"` and `"Music From www.ModArchive.com"`. The music player
  classes are `RadicalMod` and `ModSlayer`.
- **Explicit copyright string:** `ModSlayer.COPYRIGHT` (`ModSlayer.java:5`) is
  the empty string; the bytecode carries no copyright line to reproduce.
- **Year:** dated **2006** — the earliest Wayback Machine capture of the
  exact distributed jar (2006-05-05, digest-identical through 2007-07-02).
  No date is encoded in the jar itself.
- **Source:** the Wayback Machine's captures of
  `http://www.radicalplay.com/madness/mad.jar`, obtained during this
  project's recovery pass (see `PROVENANCE.md` for the digest proof).
- **Permission:** none has been sought from the rights holder. This is a
  non-commercial preservation project.
- **Rights reservation:** content © Omar Waly / Radicalplay.com, its
  original authors. All other rights in the original content are reserved
  by its authors; this project asserts no license over it.

## Liberation Sans Bold

`tools/fonts/LiberationSans-Bold.ttf` is shipped for reproducible text
rasterization and is licensed under the **SIL Open Font License, Version 1.1**.
The license and attribution files travel with the font:

- `tools/fonts/LICENSE-LiberationSans-Bold.txt` — begins "Digitized data
  copyright (c) 2010 Google Corporation with Reserved Font Arimo, Tinos and
  Cousine. Copyright (c) 2012 Red Hat, Inc. with Reserved Font Name
  Liberation", followed by the full SIL OFL 1.1 text.
- `tools/fonts/AUTHORS-LiberationSans-Bold.txt` — lists the contributors
  (Vishal Vijayraghavan and Pravin Satpute, Red Hat, Inc.; Steve Matteson,
  original designer, Ascender, Inc.).
- `tools/fonts/fontconfig.properties` — the logical-font mapping used by the
  pixel gate, not a license file.

## Third-party test fixture

`reference/external/catb0t/OBJ/mycars/Simple Car.rad` is a development car model from catb0t's public `need-for-madness-source` tree (github.com/catb0t/need-for-madness-source), shipped solely as a byte-parser fixture for the Java and Rust gates; credit to the catb0t project and the original authors, with no rights asserted by this project.

## mad.jar — original distributed artifact

`mad.jar` is the unmodified applet archive used as the ground truth for the
port and as the input to the decompilation toolchain. Its SHA-256 is recorded
in `PROVENANCE.md`, along with the Wayback CDX digest proof that the shipped
bytes are exactly what the site served from 2006-05-05 through 2007-07-02.
