// Conventional Commits: the type drives the release (`feat` a minor, `fix` and `perf` a patch).
// Two house rules ride along as a local plugin: no em dashes, no AI trailers.
const AI_TRAILER = /co-authored-by:.*(claude|codex|kimi|copilot|anthropic|openai)|generated with/i;

export default {
	extends: ['@commitlint/config-conventional'],
	plugins: [
		{
			rules: {
				'no-em-dash': ({ raw }) => [!raw.includes('\u2014'), 'no em dashes: a comma, a colon or a full stop says it'],
				'no-ai-trailer': ({ raw }) => [
					!AI_TRAILER.test(raw),
					'no AI or tool co-author trailer, no "generated with" footer: a commit ends at its content'
				]
			}
		}
	],
	rules: {
		'type-enum': [2, 'always', ['feat', 'fix', 'docs', 'chore', 'refactor', 'test', 'perf']],
		'no-em-dash': [2, 'always'],
		'no-ai-trailer': [2, 'always']
	}
};
