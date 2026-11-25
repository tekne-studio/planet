# One image is one version of planet: the Go binary with the module of world
# halves and the arcade's judge in it, the page as files, the engine's WASM and the default asset
# set. The three builds run on the machine that builds, whatever the image is
# for: the engine, the module and the page are the same on every
# architecture, and Go cross compiles.
#
#   docker build -t ghcr.io/tekne-studio/planet:<version> --build-arg VERSION=<version> .

# Bun for the machine that builds: the engine's scripts run on it in the
# engine stage, whatever the image is for.
FROM --platform=$BUILDPLATFORM oven/bun:1 AS bun

# The engine: shell-web compiled to WASM, with the bindings the page loads.
# And the module: the plugins' world halves as one WASM file the server
# embeds. On the compiler rust-toolchain.toml names, the same version.
FROM --platform=$BUILDPLATFORM rust:1.100.0 AS engine
WORKDIR /src
RUN rustup target add wasm32-unknown-unknown
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun
# wasm-bindgen-cli at the version the engine's lock names.
COPY Cargo.toml Cargo.lock ./
COPY scripts/wasm-bindgen.ts scripts/lib.ts scripts/
RUN bun scripts/wasm-bindgen.ts
COPY crates crates
# A plugin's crates are in its folder under plugins/. No emulator or game is
# built here: the emulator comes built, from arcade/emulator/, which
# `arcade` carries and reads its numbers from as it builds, and the games
# this version ships come as their ROMs, from arcade/roms/.
COPY plugins plugins
COPY arcade/emulator arcade/emulator
RUN --mount=type=cache,target=/usr/local/cargo/registry \
	--mount=type=cache,target=/src/target \
	cargo build --release --target wasm32-unknown-unknown -p shell-web -p module \
	&& wasm-bindgen --target web --out-dir /pkg target/wasm32-unknown-unknown/release/shell_web.wasm \
	&& cp target/wasm32-unknown-unknown/release/module.wasm /world.wasm

# What the server judges a cartridge by: the pinned emulator's manifest and
# its runner, checked against its hash, which the server embeds.
FROM bun AS judge
WORKDIR /src
COPY scripts/lib.ts scripts/built.ts scripts/emulator.ts scripts/
COPY arcade/emulator arcade/emulator
RUN bun scripts/emulator.ts --judge /judge

# The page: SvelteKit built into plain files, the asset set beside them.
FROM --platform=$BUILDPLATFORM oven/bun:1 AS web
WORKDIR /src
COPY package.json bun.lock ./
COPY apps/web/package.json apps/web/
# A plugin's panel is a package of the workspace, in the plugin's folder.
COPY plugins plugins
RUN bun install --frozen-lockfile
COPY apps/web apps/web
# `apps/web/static/assets` is a link to this folder: the build copies the set,
# with the ROMs this version ships written into it, each checked against its
# hash (scripts/roms.ts).
COPY assets assets
COPY scripts/lib.ts scripts/built.ts scripts/emulator.ts scripts/roms.ts scripts/
COPY arcade/emulator arcade/emulator
COPY arcade/roms arcade/roms
COPY --from=engine /pkg apps/web/src/lib/engine/pkg
# The emulator's parts beside the engine, as `bun run wasm` puts them, each
# checked against its hash; the ROMs' titles are read by it.
RUN bun scripts/emulator.ts apps/web/src/lib/engine/pkg/emulator && bun scripts/roms.ts
RUN bun run --cwd apps/web build
# The fields and the engine travel as brotli, a sibling beside each.
COPY scripts/brotli.ts scripts/
RUN bun scripts/brotli.ts apps/web/build

# The server: one static binary for the architecture the image is for.
FROM --platform=$BUILDPLATFORM golang:1.26 AS server
ARG TARGETOS=linux
ARG TARGETARCH
ARG VERSION=dev
WORKDIR /src/server
COPY server/go.mod server/go.sum ./
RUN go mod download
COPY server ./
COPY --from=engine /world.wasm internal/module/world.wasm
COPY --from=judge /judge internal/arcade/emulator
RUN CGO_ENABLED=0 GOOS=$TARGETOS GOARCH=$TARGETARCH \
	go build -trimpath -ldflags "-s -w -X main.version=$VERSION" -o /planet ./cmd/planet

FROM alpine:3
# The world folder belongs to the one user the server runs as. A volume
# mounted here for the first time takes this ownership, so mundos mounts a
# generation's folder at /world, and the copy into a next generation too.
RUN adduser -D -u 1001 planet && mkdir /world && chown planet:planet /world
COPY --from=server /planet /usr/local/bin/planet
COPY --from=web /src/apps/web/build /web
ENV HOST=0.0.0.0 \
	PORT=3000 \
	WORLD_DIR=/world \
	WEB_DIR=/web
USER planet
VOLUME /world
EXPOSE 3000
# mundos reads this to say a generation is running.
HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=5 \
	CMD wget -qO /dev/null "http://127.0.0.1:${PORT}/api/health" || exit 1
ENTRYPOINT ["planet"]
CMD ["serve"]
