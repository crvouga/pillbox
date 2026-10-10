# Asset provenance

All bundled third-party models and source sound recordings are from Kenney and
released under CC0. Original license notices are in `licenses/`. The game serves
these files locally; it makes no runtime request to an asset marketplace or TTS API.

| Source                                                                    | Bundled use                                                   | Processing                                   |
| ------------------------------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------- |
| [Blaster Kit 2.1](https://kenney.nl/assets/blaster-kit)                   | Six first-person/third-person/pickup weapons                  | Selected original GLBs and their color atlas |
| [Sci-Fi Sounds](https://kenney.nl/assets/sci-fi-sounds)                   | Weapon fire, engines, machinery, shields, pickups and reloads | OGG converted to AAC/M4A for WebKit playback |
| [Impact Sounds](https://kenney.nl/assets/impact-sounds)                   | Footsteps, impacts and landings                               | OGG converted to AAC/M4A                     |
| [Voiceover Pack Fighter](https://kenney.nl/assets/voiceover-pack-fighter) | Prerecorded match announcements                               | Selected clips converted to AAC/M4A          |
| [Voiceover Pack](https://kenney.nl/assets/voiceover-pack)                 | Prerecorded objective announcements                           | Selected clips converted to AAC/M4A          |

Weapon GLBs use `weapons/Textures/colormap.png`. Keep that atlas and its relative
paths intact. Station models were removed with the grid map overhaul; their
original download provenance and license notice remain below.

Downloaded source archive SHA-256:

```text
blaster-kit:       91e3093e95427d59625e7e2ce2d0399b861600160fd0b4ada7714796b67cea8c
space-station-kit: 215e79bd5415cff93665183390f0343ed9acf87780306331013b78520170c6d8
sci-fi-sounds:     119340f351a5098ad814f78719438c0da355a9ce8a4c8a3af6a8d48aa3d49e04
impact-sounds:     029d734af1582474edf3a694d1b0cebc97c1c152f2f39fa34d4c2bafc5de77f8
```

`audio/voice/manifest.json` is empty until a generated replacement pack exists.
The client prefers entries from that manifest and otherwise plays the CC0 clips.
See `scripts/generate-announcer.py` for offline ElevenLabs generation; no API key
belongs in this directory or in browser code.
