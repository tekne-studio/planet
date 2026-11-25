// The cart an engine command names, `bun run cart` and `bun run shot
// --cabinet`: a game this version ships, by its name, as its ROM
// (`roms.ts`), or a built cart, a `.wasm`, as it is. The engine builds no
// cart: the console builds one (`bun run cart` and `bun run workbench` in
// TKN-32), and the engine runs the file it writes.
import { kept, named } from './built';
import { listed, ROMS } from './roms';

/** The path of the file `asked` names. */
export function cartFile(asked: string): string {
	if (asked.endsWith('.wasm')) return asked;
	const shipped = listed().find(({ name }) => name === asked);
	if (!shipped) {
		const names = listed().map(({ name }) => name);
		throw new Error(`no ROM named ${asked}: one of ${names.join(', ')}, or a built cart's .wasm`);
	}
	kept(ROMS, shipped.sha256, 'wasm');
	return `${ROMS}/${named(shipped.sha256, 'wasm')}`;
}
