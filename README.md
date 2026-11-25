# Planet

A finite, spherical voxel world that people and AI agents walk, fly, drive
and build in together. Free software, built from scratch.

- One Rust client on wgpu, the same on the desktop and in a browser tab
  (WebAssembly), and one Go server. One world per instance.
- The world is a recipe: its folder holds only what someone changed.
  Offline mode is permanent.
- What is done with the world is a plugin: chat, build, items, ride and
  flashlight ship today, switched in `plugins.json`.
- Arcade cabinets in the world run games made for
  [TKN-32](https://github.com/tekne-studio/tkn-32), a fantasy console of
  its own: the engine carries its emulator built, in `arcade/emulator/`.

## Layout

| Path | What |
|---|---|
| `crates/` | the core: topology, world generation, voxels, rendering, the client and its shells |
| `plugins/` | one folder a plugin: its client and world halves, its panel |
| `server/` | the Go server: one world, its socket, its folder |
| `apps/web/` | the page (SvelteKit) that loads the engine's WebAssembly |
| `proto/` | the wire's schemas; the generated types are committed |
| `assets/` | the default asset set; its terms are in [NOTICE](NOTICE) |
| `arcade/emulator/` | the emulator of TKN-32 the cabinets run, built, each file named by its hash, `emulator.json` naming each |
| `arcade/roms/` | the ROMs a version ships, each game's built file named by its hash, listed in order in `roms.toml` |

## Run it

Needs Rust at the version `rust-toolchain.toml` names (rustup takes it from
there), Go and Bun.

```sh
bun run setup      # dependencies, git hooks, the wasm target, wasm-bindgen-cli, the protocol generators
bun run dev        # the server on :8190 and the page on :5173, or the next free ports (builds the engine first)
bun run desktop    # the offline explorer in a native window
bun run shot --out out/orbit.png --altitude 30000 --pitch -60 --boom 50
bun run check      # everything CI runs
```

- Alone, whoever opens the page is an admin. The first visit founds the
  world: a seed, Earth or generated, its knobs. It is kept in
  `server/world`; delete that folder for an unfounded world again.
- `.env.example` names every variable a script reads; copy it to `.env`.
- Keys: `W A S D` move, `Space` jump, rise or leap from the water, `C`
  descend or dive, `Shift` run, `F` walk or fly, `V` next avatar, `R` new
  seed (desktop), wheel zoom, `Esc` release the pointer.
- `bun run cart rocks --window` plays a ROM in a window, the keys its
  buttons; a run with no window, a strip of PNGs, and the workbench are
  TKN-32's commands.

## The image

One image a version: `ghcr.io/tekne-studio/planet:<version>`, for amd64 and
arm64. It holds the Go binary, the page's files, the engine's WebAssembly,
the default asset set and the ROMs, and runs one process as a user of its
own.

```sh
docker build -t planet --build-arg VERSION=<version> .
docker run -p 3000:3000 -v planet-world:/world -e PLANET_DEV_LEVEL=admin planet
```

- Commands: `planet serve` (the default), `planet copy <from> <to>` (a
  world folder into an empty one, the source read-only), `planet version`.
- `/world` is the world folder: the recipe, the plugins switched, a SQLite
  file for each owner that keeps something, and the files builders sent.
  It is the whole world.
- `GET /api/health` answers once the server listens; the image's
  `HEALTHCHECK` asks it.

### Hosted by mundos

mundos hosts Planet worlds: it runs the image, mounts the world folder at
`/world` and sets these:

| Variable | What |
|---|---|
| `PORT` | where the server listens inside the container: 3000, as the image says |
| `MUNDOS_PUBLIC_KEY` | the Ed25519 public key that checks tokens: PEM, its newlines escaped |
| `PUBLIC_MUNDOS_URL` | mundos's origin, where people sign in |
| `PUBLIC_MUNDOS_WORLD` | the world's name, the token's audience |

- The three `MUNDOS` variables are set together or not at all. With them,
  people arrive signed and mundos says each one's level.

### What the server reads

| Variable | Default | What |
|---|---|---|
| `HOST` | `127.0.0.1`; the image `0.0.0.0` | the address it listens on |
| `PORT` | `8090`; the image `3000` | the port it listens on |
| `WORLD_DIR` | `world`; the image `/world` | the world folder |
| `WEB_DIR` | unset; the image `/web` | the page's files, served by the server |
| `ASSETS_DIR` | `WEB_DIR/assets` | the asset set: its `fields/` and the ROMs in `carts/` |
| `PLANET_DEV_LEVEL` | `anonymous` | alone, every session's level: `anonymous`, `signed_in`, `builder` or `admin`; never beside `MUNDOS_PUBLIC_KEY` |

- `bun run dev` and `bun run server` set `PLANET_DEV_LEVEL=admin` and
  `ASSETS_DIR` to the repository's `assets/` unless `.env` says otherwise.

## Releases

- Versions count from 0.0.1, the tag on the history's first commit.
- Conventional Commits decide each version: `feat` a minor, `fix` and
  `perf` a patch. While the major is 0, `BREAKING CHANGE` is a minor too:
  1.0.0 is declared, never reached by a commit type
  ([release.config.js](release.config.js)).
- Work lands on `dev`, which publishes prereleases of the next version:
  `<version>-dev.N`.
- Merging `dev` into `main` publishes the plain version.
- Each release is a git tag `v<version>`, release notes on GitHub, and the
  image under the version's number.

## License

- Planet is free software under the GNU General Public License, version 3
  only: [LICENSE](LICENSE).
- `arcade/emulator/`, TKN-32's emulator built, is under the MIT licence,
  given in [NOTICE](NOTICE).
- `arcade/roms/`: each ROM as `roms.toml` says, under a licence or with its
  rights reserved. The Kings of Rio '95 ROM is Tekne Studio's, all rights
  reserved, and not under the engine's licence: no licence is granted for
  it until its licence is chosen. Planet carries and runs it with Tekne
  Studio's permission ([NOTICE](NOTICE)).
- The default asset set's files and their terms: [NOTICE](NOTICE).

Copyright (C) 2026 Tekne Studio.
