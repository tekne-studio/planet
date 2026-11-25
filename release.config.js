// Semantic Release. Versions count from 0.0.1, the tag on the history's
// first commit, and stay under 1.0.0 until 1.0.0 is declared.
//
// - `dev` publishes prereleases of the next version, `<version>-dev.N`
// - merging `dev` into `main` publishes the plain version
// - `feat` bumps the minor, `fix` and `perf` the patch, as the analyzer's
//   defaults say; a BREAKING CHANGE bumps the minor too, by the rule below.
//   1.0.0 is declared, never reached by a commit type: the commit that
//   declares it takes the rule out, and its BREAKING CHANGE makes 1.0.0.
// scripts/release.test.ts runs these rules on a repository of its own.
export default {
	branches: ['main', { name: 'dev', prerelease: 'dev' }],
	tagFormat: 'v${version}',
	plugins: [
		[
			'@semantic-release/commit-analyzer',
			{ preset: 'conventionalcommits', releaseRules: [{ breaking: true, release: 'minor' }] }
		],
		['@semantic-release/release-notes-generator', { preset: 'conventionalcommits' }],
		'@semantic-release/github'
	]
};
