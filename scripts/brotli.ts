// Writes a brotli sibling beside each heavy file of the built page: the fields
// and the engine. The server sends `<file>.br` in place of `<file>` to a
// browser that reads brotli (`server/internal/api/front_end.go`), and nothing
// in front of it compresses: mundos's proxy passes bytes as they are.
//   bun scripts/brotli.ts apps/web/build
import { Glob } from 'bun';
import { brotliCompressSync, constants } from 'node:zlib';

const root = process.argv[2];
if (!root) throw new Error('usage: bun scripts/brotli.ts <built page folder>');

const mb = (bytes: number) => (bytes / 1e6).toFixed(2);

for await (const path of new Glob('**/*.{field,wasm}').scan(root)) {
	const file = `${root}/${path}`;
	const bytes = await Bun.file(file).bytes();
	const packed = brotliCompressSync(bytes, {
		params: {
			[constants.BROTLI_PARAM_QUALITY]: constants.BROTLI_MAX_QUALITY,
			// The widest window a browser decodes: a field's planes repeat
			// across its faces, megabytes apart.
			[constants.BROTLI_PARAM_LGWIN]: constants.BROTLI_MAX_WINDOW_BITS,
			[constants.BROTLI_PARAM_SIZE_HINT]: bytes.length
		}
	});
	await Bun.write(`${file}.br`, packed);
	console.log(`${path}: ${mb(bytes.length)} MB, ${mb(packed.length)} as brotli`);
}
