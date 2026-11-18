# assets/

The server-side asset bundle the original applet fetches at runtime
(see `notes/ARCHITECTURE.md` — "What comes from where"). Recovered from
Wayback Machine captures of `http://www.radicalplay.com/madness/...` by
`tools/fetch-assets.sh`; byte-identity to each archived capture is proven in
[`PROVENANCE.md`](PROVENANCE.md) (sha1/digest cross-check per file).

Game data is entirely data-driven — the engine in `mad.jar` contains zero
track/car geometry or music; everything comes from here:

```
assets/
├── graphics/
│   ├── models.zipo     43 raw-model files (*.rad): 10 cars + 33 track pieces
│   │                   (parsed by the original ContO(byte[],...) constructor)
│   ├── images1..4.zipo zipped GIFs for menus/HUD (lap.gif, cars.gif, ...)
├── music/
│   ├── stage1..11.zipo one stage*.mod ProTracker-style module per stage
│   ├── cars.zipo       car-select music
│   └── stages.zipo     menu/other music
├── stages/
│   └── 1..11.txt       stage definitions (text commands, see ARCHITECTURE.md)
├── PROVENANCE.md       per-file capture timestamp + digest verification
└── README.md           this file
```

Notes:

- **`.zipo` files are plain zip archives** (the game streams them from the
  server via `ZipInputStream`). Inner names use the original era's spelling
  (e.g. `canyenaro.rad`).
- Files dated 2004–2005 inside the zips match the mad.jar class dates, i.e.
  this is the same era as the binary in this repo, not the later standalone
  versions.
- **Completeness cross-check:** every one of the 46 non-bundled GIF names
  the decompiled code loads is present in the recovered zips (the other 3
  GIFs — `hello/loadbar/sign` — ship inside mad.jar itself).
- The Rust port should read these directly: unzip-in-memory + parse `.rad`
  like `ContO`, `.mod` with the `Mod.LoadMod` logic, `.txt` like
  `GameSparker.loadstage()`. `PROVENANCE.md` is the go-to reference if any
  file is ever questioned.
- To refetch/regenerate: `tools/fetch-assets.sh` (idempotent; deletes +
  reruns per file as needed; newer captures are never preferred over the
  earliest ones — era stability is deliberate).
