// The release rules, `release.config.js`, run by semantic-release itself,
// dry and offline, on a repository of its own: a bare one as its remote,
// the history's first commit on `main` tagged v0.0.1, and `dev` on top. Each
// test says what a push of `dev` or `main` would release.
import { afterAll, describe, expect, setDefaultTimeout, test } from 'bun:test';
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { Writable } from 'node:stream';
import semanticRelease, { type Options } from 'semantic-release';
import config from '../release.config.js';

// A run loads its plugins, reads the history and asks the remote: half a
// second here, more on a busy runner, and a test runs a few.
setDefaultTimeout(30_000);

/** The repositories the tests made, removed after them. */
const made: string[] = [];
afterAll(() => {
	for (const dir of made) rmSync(dir, { recursive: true, force: true });
});

/** The rules as CI runs them, without the plugin that publishes on GitHub. */
const offline: Options = {
	...config,
	plugins: config.plugins.filter((plugin) => plugin !== '@semantic-release/github')
};

/** The rules without the hold under 1.0.0: what declaring 1.0.0 would release. */
const declared: Options = {
	...offline,
	plugins: offline.plugins!.map((plugin) =>
		Array.isArray(plugin) && plugin[0] === '@semantic-release/commit-analyzer'
			? [plugin[0], { preset: 'conventionalcommits' }]
			: plugin
	)
};

const silent = () => new Writable({ write: (_chunk, _encoding, done) => done() });

/** A repository whose first commit is v0.0.1 on `main`, with `dev` checked out on it. */
function repository() {
	const dir = realpathSync(mkdtempSync(`${tmpdir()}/release-`));
	made.push(dir);
	const remote = `${dir}/remote.git`;
	const work = `${dir}/work`;
	mkdirSync(work);
	// Git alone, whatever the machine or a hook that runs this sets.
	const env = {
		PATH: process.env.PATH!,
		HOME: dir,
		GIT_CONFIG_GLOBAL: '/dev/null',
		GIT_CONFIG_NOSYSTEM: '1',
		GIT_AUTHOR_NAME: 'Planet',
		GIT_AUTHOR_EMAIL: 'planet@example.com',
		GIT_COMMITTER_NAME: 'Planet',
		GIT_COMMITTER_EMAIL: 'planet@example.com'
	};
	const git = (...args: string[]) => {
		const run = Bun.spawnSync(['git', ...args], { cwd: work, env });
		if (run.exitCode !== 0) throw new Error(`git ${args.join(' ')}: ${run.stderr}`);
	};
	let commits = 0;
	const commit = (message: string) => {
		writeFileSync(`${work}/file`, `${++commits}\n`);
		git('add', '-A');
		git('commit', '-q', '-m', message);
		git('push', '-q', 'origin', 'HEAD');
	};
	git('init', '-q', '--bare', '-b', 'main', remote);
	git('init', '-q', '-b', 'main');
	git('remote', 'add', 'origin', remote);
	commit('refactor: the first commit');
	git('tag', 'v0.0.1');
	git('push', '-q', 'origin', 'v0.0.1');
	git('checkout', '-q', '-b', 'dev');
	git('push', '-q', 'origin', 'dev');
	return {
		git,
		commit,
		/** The version a push of the branch checked out would release, or none. */
		next: async (options = offline) => {
			const result = await semanticRelease(
				{ ...options, dryRun: true, ci: false, repositoryUrl: `file://${remote}` },
				{ cwd: work, env, stdout: silent(), stderr: silent() }
			);
			return result ? result.nextRelease.version : null;
		}
	};
}

describe('dev, after the first release', () => {
	test('releases nothing until a commit asks for a version', async () => {
		const r = repository();
		expect(await r.next()).toBeNull();
		r.commit('docs: a word');
		expect(await r.next()).toBeNull();
	});

	test('publishes a prerelease of the next patch for a fix, of the next minor for a feature', async () => {
		const r = repository();
		r.commit('fix: a fix');
		expect(await r.next()).toBe('0.0.2-dev.1');
		r.commit('feat: a feature');
		expect(await r.next()).toBe('0.1.0-dev.1');
	});
});

describe('a breaking change', () => {
	test('bumps the minor, said by a ! or by its footer, never to 1.0.0', async () => {
		const r = repository();
		r.commit('feat!: a change that breaks');
		expect(await r.next()).toBe('0.1.0-dev.1');
		const footer = repository();
		footer.commit('fix: a fix\n\nBREAKING CHANGE: it breaks');
		expect(await footer.next()).toBe('0.1.0-dev.1');
	});

	test('bumps the minor on main too, from any 0.x', async () => {
		const r = repository();
		r.git('checkout', '-q', 'main');
		r.git('tag', 'v0.3.2');
		r.git('push', '-q', 'origin', 'v0.3.2');
		r.commit('feat!: a change that breaks');
		expect(await r.next()).toBe('0.4.0');
	});

	test('makes 1.0.0 once the rule is taken out, the declaration', async () => {
		const r = repository();
		r.commit('feat!: 1.0.0');
		expect(await r.next(declared)).toBe('1.0.0-dev.1');
	});
});
