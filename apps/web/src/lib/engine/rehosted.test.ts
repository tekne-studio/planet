import { describe, expect, test } from 'bun:test';
import { rehosted } from './rehosted';

/** A host that counts what it hosts; each hosted one breaks for good once used to throw. */
function counting() {
	let hosts = 0;
	const host = async () => {
		const n = ++hosts;
		let broken = false;
		return {
			n,
			call(throws: boolean) {
				if (broken) throw new Error(`host ${n} is broken`);
				if (throws) {
					broken = true;
					throw new Error(`host ${n} trapped`);
				}
				return n;
			}
		};
	};
	return { host, hosts: () => hosts };
}

describe('rehosted', () => {
	test('hosts once while every use answers', async () => {
		const { host, hosts } = counting();
		const use = rehosted(host);
		expect(await use((h) => h.call(false))).toBe(1);
		expect(await use((h) => h.call(false))).toBe(1);
		expect(hosts()).toBe(1);
	});

	test('hosts again after a use that throws, which still throws', async () => {
		const { host, hosts } = counting();
		const use = rehosted(host);
		await expect(use((h) => h.call(true))).rejects.toThrow('host 1 trapped');
		expect(await use((h) => h.call(false))).toBe(2);
		expect(hosts()).toBe(2);
	});

	test('gives null each use when there is nothing to host', async () => {
		let tries = 0;
		const use = rehosted(async () => {
			tries++;
			throw new Error('not copied');
		});
		expect(await use(() => 1)).toBeNull();
		expect(await use(() => 1)).toBeNull();
		expect(tries).toBe(1);
	});
});
