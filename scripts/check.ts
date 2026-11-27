// What CI runs, locally, in the same order.
import { $ } from 'bun';
import { ROOT } from './lib';

$.cwd(ROOT);
await $`cargo fmt --all --check`;
await $`cargo clippy --workspace --all-targets -- -D warnings`;
await $`cargo test --workspace`;
// The synth's samples are the same bits on wasm32, under node.
await $`cargo test -p synth --target wasm32-unknown-unknown`;
// So is what lives: gaia's goldens, the same hashes on wasm32; and its
// legs hold there, where the world draws them in a browser.
await $`cargo test -p gaia --target wasm32-unknown-unknown --test golden --test legs`;
await $`cargo build --release --target wasm32-unknown-unknown -p shell-web`;
// The server embeds the module: built before Go is.
await $`bun scripts/module.ts`;
await $`go vet ./...`.cwd(`${ROOT}/server`);
await $`go test ./...`.cwd(`${ROOT}/server`);
await $`bun run --cwd apps/web check`;
// The emulator and the ROMs come built (arcade/): each file hashes to its
// name and its list reads (scripts/*.test.ts), and the emulator's cases
// pass on the native host (arcade-host's tests, above). That the pin is the
// console's build is the console's to prove, where its source is.
await $`bun test ./apps/web/src ./plugins ./scripts`;
// The wire types in the tree are the schemas': a stale generation is a
// schema change nobody built. The same holds for the registries of plugins
// and the config they come from.
await $`bun scripts/proto.ts`;
await $`bun scripts/plugins.ts`;
await $`git diff --exit-code -- crates/protocol/src/gen server/internal/protocol plugins/*/world/src/gen crates/plugins-client crates/plugins-world apps/web/src/lib/plugins/index.ts buf.yaml`;
console.log('all checks pass');
