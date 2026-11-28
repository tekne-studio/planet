/**
 * What this browser keeps of a person's choices: the avatar worn, a visitor's
 * name. A convenience: a browser that keeps them spares the person choosing
 * again.
 */
const PREFIX = 'planet.';

export function kept(name: string): string | null {
	try {
		return localStorage.getItem(PREFIX + name);
	} catch {
		// storage blocked: the person chooses again
		return null;
	}
}

/** Whether this browser keeps what is written: a choice asked once is never asked again only where it does. */
export function keeps(): boolean {
	try {
		const probe = PREFIX + 'kept';
		localStorage.setItem(probe, '1');
		localStorage.removeItem(probe);
		return true;
	} catch {
		return false;
	}
}

/** Calls `heard` with each value another page of this site keeps under `name`, until the returned stop. */
export function onKept(name: string, heard: (value: string | null) => void): () => void {
	const listen = (event: StorageEvent) => {
		if (event.key === PREFIX + name) heard(event.newValue);
	};
	addEventListener('storage', listen);
	return () => removeEventListener('storage', listen);
}

/** Lets go of what is kept under `name`. */
export function forget(name: string): void {
	try {
		localStorage.removeItem(PREFIX + name);
	} catch {
		// storage blocked: nothing was kept
	}
}

export function keep(name: string, value: string): void {
	try {
		localStorage.setItem(PREFIX + name, value);
	} catch {
		// best effort: the choice then lasts for this visit
	}
}
