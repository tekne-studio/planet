/**
 * An emulator hosted when first used, and hosted again after a use that
 * throws: a runner that traps keeps its state half changed and answers
 * nothing more, so the next use starts from a fresh one. The use that threw
 * still throws. Null, each use, while there is none to host.
 */
export function rehosted<T>(host: () => Promise<T>): <R>(use: (hosted: T) => R) => Promise<R | null> {
	let held: Promise<T | null> | undefined;
	return async (use) => {
		const holding = (held ??= host().catch(() => null));
		const hosted = await holding;
		if (hosted === null) return null;
		try {
			return use(hosted);
		} catch (error) {
			// Another use may have hosted a fresh one already.
			if (held === holding) held = undefined;
			throw error;
		}
	};
}
