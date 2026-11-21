<p align="center">
  <img src="docs/media/banner.png" alt="NOCTILUCA: sound becomes light" width="100%">
</p>

<p align="center">
  <b>A live club visualizer for macOS.</b><br>
  It listens to the room, finds the kick, reads the vibe, and plays the night<br>
  on screens, projectors, TV boxes and DMX lights, in sync.<br>
  Techno, house, trance: anything built on a kick.
</p>

<p align="center">
  <img alt="macOS 13+" src="https://img.shields.io/badge/macOS-13%2B-01060f?style=flat-square&labelColor=01060f&color=2e7cf6">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-01060f?style=flat-square&labelColor=01060f&color=2e7cf6">
  <img alt="GPU: wgpu / Metal" src="https://img.shields.io/badge/GPU-wgpu%20%C2%B7%20Metal-01060f?style=flat-square&labelColor=01060f&color=2e7cf6">
  <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-01060f?style=flat-square&labelColor=01060f&color=2e7cf6">
</p>

<p align="center">
  <img src="docs/media/noctiluca.gif" alt="Noctiluca playing: a mirror ball turning to the music" width="720">
</p>

> *Noctiluca* is a sea plankton that glows when the water moves. This one glows when the music does.

---

## `01` · What it does

- **Listens.** A microphone (or any input) is enough. It tracks the tempo and the beat grid, hears every kick, and knows the section: groove, build, drop, breakdown.
- **Reads the vibe.** Dark or bright, calm or intense, smooth or raw, measured over the last seconds, and the looks follow it.
- **Directs itself.** A director picks themes, backgrounds, patterns, 3D objects, atmospheres and effects, changing on phrase boundaries, cross-fading on the beat. Or take it over by hand.
- **Plays anywhere.** The Mac's screen, up to four outputs (projectors, displays), Android TV boxes over Wi-Fi, and DMX lights over USB.
- **Maps the room.** Up to eight surfaces, each warped onto a wall, a pillar or a ceiling, with a calibration grid that is projected right onto them.
- **Keeps people safe.** Never more than three flashes a second, whatever the music does.

## `02` · Looks

<table>
  <tr>
    <td><img src="docs/media/look1.jpg" alt="A chrome mirror ball in a cyan tunnel"></td>
    <td><img src="docs/media/look2.jpg" alt="Amber light beams over a laser grid"></td>
    <td><img src="docs/media/look3.jpg" alt="Red shards in liquid"></td>
  </tr>
  <tr>
    <td><img src="docs/media/look4.jpg" alt="Glass spheres over pale rings"></td>
    <td><img src="docs/media/look5.jpg" alt="A violet corridor in a lightning storm"></td>
    <td><img src="docs/media/look6.jpg" alt="A golden knot over fire"></td>
  </tr>
</table>

Six themes (Red room, White corridor, Terminal, Ultraviolet, Amber, Rainbow) built from:

| | |
|---|---|
| **Backgrounds** | tunnel · laser grid · liquid · 3D lattice · void |
| **Patterns** | rings · scope · kaleidoscope · spectrum bars · stripes · frames · radar · square spiral · halftone |
| **3D objects** | mirror ball · skull · prism · gem · knot · eye · monolith · chain · speakers · crystal · planet · corridor (chrome or glass) |
| **Atmospheres** | storm with lightning · smoke · fire · moving-head beams · aurora |
| **Effects** | trails · zoom · kaleido · drift · CRT · prism · floor · VHS · thermal camera · film |
| **Palettes** | ten: five in colour, five of one colour on black |

On a build the camera pushes in; on the drop everything hits at once.

## `03` · The app

<p align="center"><img src="docs/media/app-live.png" alt="The Live page: program, pads, inspector and the set's timeline"></p>

A control window in the spirit of pro audio and video apps: a toolbar with the transport and the beat, a sidebar of sections, an inspector.

- **Live** · the program, pads (Director: Auto or Held, Next look, Palette, Strobe, Lights out, Smoke, Grid, Full screen), an inspector for the look, the sound and the outputs, and a timeline of the last twenty minutes: sections, looks, light effects.
- **Screens** · projection mapping.
- **Lights** · DMX.
- **Network screens** · TV boxes on the Wi-Fi.
- **Venues** · whole set-ups saved per place.
- **Settings** · flash safety, sync, quality.

## `04` · Projection mapping

<table>
  <tr>
    <td width="50%"><img src="docs/media/app-screens.png" alt="The Screens page: three keystoned surfaces on one projector"></td>
    <td width="50%"><img src="docs/media/output-grid.jpg" alt="The calibration grid projected on three surfaces"></td>
  </tr>
</table>

- Up to **8 surfaces** on up to **4 outputs**; split them across projectors or keep several on one.
- **Transform** (move, rotate with 15° snaps, scale about the middle) and **Corner pin** (corners and edges) tools; arrows move one pixel; true perspective (homography), never a stretch.
- One or two **Main** surfaces; the others **Same** (a copy) or **Complement**: same colours and beat, a different picture: **Mirror** (the stage symmetric), **Chase** (a soft light running across the surfaces in the last bar of each phrase) or **Echo** (Main, one beat later).
- For **walls seen from the side and ceilings**: turn the picture inside its corners, and give a surface its real shape so circles stay round and 3D keeps its proportions.
- While mapping, the projector shows the show with the selected surface outlined on the wall itself.

## `05` · Network screens

Any **Android TV box** on the Wi-Fi becomes an output, mapped like a projector. Noctiluca finds the boxes, installs its small receiver app on them (over network debugging, from the Network screens page), and streams H.264 from the Mac's hardware encoder. The two clocks are kept in step and every frame is drawn ahead by the measured delay, so a TV across the room lands on the same beat as the projector.

The receiver lives in [`android/`](android/): plain Android, no libraries, hardware decoding, frames shown at the exact moment asked for, HDMI game mode on request.

## `06` · DMX lights

<p align="center"><img src="docs/media/app-lights.png" alt="The Lights page: interface, effects, Drive, lights and smoke"></p>

- **USB-DMX**: Open DMX (FTDI) and Enttec DMX USB Pro style interfaces, told apart automatically.
- **Lights** as cards with their live colour: RGB, dimmer + RGB, the common 7-channel RGB spot, RGBW, a smoke machine, or any custom channel list. A **channel finder** for lights whose manual is lost.
- **Effects**: Pulse · Pop · Split · Wave · Chase · Breathe · Riser. **Auto** follows the section and the vibe and changes on phrases; or lock one.
- **Drive**: one slider from *subtle* to *wild*: how bright, how deep the dips between hits, how fast things move, how snappy the fades, how vivid the colours.
- **Strobe** hits on drops (and at the top of builds), through the same three-a-second limit. The lights' own strobe channels are never used.
- **Smoke**: by hand, or a burst on the drop, never more often than you set.

## `07` · Venues

<p align="center"><img src="docs/media/app-venues.png" alt="The Venues page" width="80%"></p>

Save everything set up for a place (outputs, surfaces, network screens, lights, safety, sync, themes, quality) and load it at the next gig there. The input device stays with the computer. Loading over unsaved changes asks first.

## `08` · Run it

Requirements: macOS 13 or later, [Rust](https://rustup.rs) 1.95 or later.

```bash
git clone https://github.com/sagistiki/noctiluca
cd noctiluca
cargo run --release
```

It opens on **Live** and listens to the microphone. Press **Start the show**, or Space.

Build the Mac app (with the TV receiver inside, if Java and the Android SDK are around):

```bash
./package_app.sh            # → ~/Documents/Noctiluca.app
```

| Want to | Run |
|---|---|
| Play a WAV file instead of listening | `cargo run --release -- --file track.wav` |
| Choose the input | `cargo run --release -- --device "BlackHole"` |
| Use a network screen and start at once | `cargo run --release -- --screen "Living room" --play` |
| No flashes at all | `cargo run --release -- --no-strobe` |
| The same show every time | `cargo run --release -- --seed 7` |
| Make the synthesized test track | `cargo run --example gen_track` |
| All the options | `cargo run --release -- --help` |

### Keys (while the show is full screen)

| Key | |
|---|---|
| `Space` | a new look now |
| `L` | hold the look (the Director to *Held*), again to let go |
| `1`–`5` · `Q`–`Y` · `Z X C` | background · pattern |
| `P` · `O` · `G` · `B` · `A` | palette · effect · 3D object · chrome or glass · atmosphere |
| `S` | strobe on or off |
| `[` `]` | visuals 10 ms later or earlier, for this room |
| `I` · `H` · `F` | next input · overlay always on · full screen |
| `Esc` · `Cmd+Q` | back to Live (the show goes on) · quit |

## `09` · Flash safety

Fast flashing can trigger seizures in people with photosensitive epilepsy. Every flash, on screen and in the lights, goes through one limiter: **at most three a second** (WCAG 2.3.1), each one only fading once it starts. Anything faster becomes a shallow shimmer. `S` turns flashes off, and so does `--no-strobe`.

## `10` · How it is built

```
src/
  analysis/      FFT, kick and beat tracking, sections, the vibe
  director.rs    the looks, and when they change
  sync.rs        the beat clock, running ahead of the program's own delay
  render/        wgpu: views, compositor (mapping), projector, readback
  stage.rs       surfaces → what each output draws
  mapping.rs     homography, corner pin, rotation
  net/           network screens: discovery, protocol, clock sync, VideoToolbox H.264
  lights.rs      the DMX light engine, effects, Drive, smoke
  dmx.rs         Open DMX / Enttec Pro over USB serial
  ui/            the control window (egui)
shaders/         WGSL, reloaded live while you edit them
assets/models/   the 3D objects (glTF), made in Blender (blender/build_models.py)
android/         the TV box receiver
docs/brand/      the icon and banners as SVG/HTML, rendered by render.sh
```

Rust 2024 · [wgpu](https://wgpu.rs) on Metal · [egui](https://github.com/emilk/egui) · cpal · rustfft · VideoToolbox. About 290 tests: `cargo test --release`.

## License

[MIT](LICENSE). The Inter font is under the SIL Open Font License ([assets/fonts/Inter-LICENSE.txt](assets/fonts/Inter-LICENSE.txt)).

<p align="center"><sub>Made for the dance floor.</sub></p>
