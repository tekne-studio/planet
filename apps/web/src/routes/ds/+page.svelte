<script lang="ts">
	import {
		Alert,
		Badge,
		Button,
		Checkbox,
		CodeInput,
		Dial,
		Dialog,
		Field,
		Fold,
		Icon,
		IconButton,
		Palette,
		VectorInput,
		WorldCard,
		Input,
		KeyBadge,
		Knob,
		Knobs,
		LangSwitch,
		Marker,
		MarkerCard,
		Page,
		PageHeader,
		Pager,
		Pairs,
		Panel,
		Ring,
		Segmented,
		Select,
		Slider,
		Spinner,
		Stack,
		Stat,
		Table,
		Tabs,
		ThemeToggle,
		Topbar,
		Veil
	} from '$lib/ds';

	// The living catalogue. A developer reference, written in the project
	// language like the code it documents; it is not product UI, so its prose
	// does not go through i18n. Components render with their real strings.

	let shadows = $state(true);
	let settled = $state(0.5);
	let settledAt = $state(0.5);
	let cover = $state(0.5);
	let reach = $state(256);
	let sectionTab = $state<'graphics' | 'game'>('graphics');
	const sectionTabs = [
		{ value: 'graphics' as const, label: 'Graphics' },
		{ value: 'game' as const, label: 'Game' }
	];
	let closed = $state<number | null>(0.4);
	let pulse = $state(0);
	let taps = $state(0);
	/** The tap's sweep, shown again and again in the states row. */
	let sweeps = $state(1);
	$effect(() => {
		const timer = setInterval(() => (sweeps += 1), 1200);
		return () => clearInterval(timer);
	});
	/** The sample marker's card, open or not, and the last choice pressed in it. */
	let markerOpen = $state(true);
	let chosen = $state('');
	const markerChoices = ['Ride', 'Call', 'Release'].map((label) => ({ label, onclick: () => (chosen = label) }));
	let curve = $state('aces');
	let game = $state('rocks');
	const games = [
		{ value: 'rocks', label: 'Rocks' },
		{ value: 'file', label: 'Load a file…', rule: true }
	];
	let colour = $state('action');
	const colours = [
		{ value: 'action', label: 'Action' },
		{ value: 'focus', label: 'Focus' },
		{ value: 'text-muted', label: 'Text muted' },
		{ value: 'bg-inset', label: 'Ground inset' }
	];
	const curves = [
		{ value: 'aces', label: 'ACES' },
		{ value: 'agx', label: 'AgX' }
	];

	const primitives = ['black', 'grey-900', 'grey-800', 'grey-600', 'grey-400', 'grey-300', 'grey-100', 'white', 'red-600', 'red-400'];
	const semantics = [
		'bg',
		'bg-inset',
		'bg-overlay',
		'text',
		'text-muted',
		'border',
		'border-strong',
		'action',
		'action-text',
		'focus',
		'danger'
	];
	const spaces = [1, 2, 3, 4, 5, 6, 7, 8];
	const speeds = [
		{ value: 'walk', label: 'Walk' },
		{ value: 'fly', label: 'Fly' }
	] as const;

	let speed = $state<(typeof speeds)[number]['value']>('walk');
	let email = $state('');
	let code = $state('3635');
	let loading = $state(false);
	let confirming = $state(false);
	let deleting = $state(false);
	let choosing = $state(false);
	let outcome = $state('');
	const pictures = [
		{ value: 'light', label: 'Light' },
		{ value: 'high', label: 'High' }
	] as const;
	let picture = $state<(typeof pictures)[number]['value']>('light');

	function wait(ms: number): Promise<void> {
		return new Promise((resolve) => setTimeout(resolve, ms));
	}

	async function pretend() {
		loading = true;
		await wait(1200);
		loading = false;
	}
</script>

<svelte:head>
	<title>Design system · planet</title>
</svelte:head>

<Topbar />

<Page>
	<PageHeader
		title="Design system"
		lede="Black and white, one typeface at one size, zero radius. Hierarchy comes from weight, case, spacing and borders. Every component below is the real one, in its states."
		trail={[{ label: 'planet', href: '/' }]}
	>
		{#snippet actions()}
			<LangSwitch />
			<ThemeToggle />
		{/snippet}
	</PageHeader>

	<div class="sections">
		<section>
			<h2>Rules</h2>
			<ul class="rules">
				<li>Components reference semantic tokens only. Primitives appear in <code>tokens.css</code> and nowhere else.</li>
				<li>One font size: <code>--fs</code> (10px). Never scale type; use weight, upper case, letter spacing, borders.</li>
				<li>One radius: <code>--radius</code> (0). Borders are <code>--bw</code> wide.</li>
				<li>Theme: <code>light-dark()</code> driven by <code>color-scheme</code>. No choice follows the OS; the <code>theme</code> cookie pins it and the server stamps <code>&lt;html data-theme&gt;</code>, so nothing flashes.</li>
				<li>
					Danger is the one colour, a single restrained red. An error or a destructive action must never read as ordinary
					chrome. Colour is never the only carrier: alerts and field errors lead with a <code>!</code> marker, destructive
					buttons say what they destroy and always confirm in a Dialog.
				</li>
				<li>Every wait gives a signal: a Button that waits for the server takes <code>loading</code>.</li>
				<li>No user visible literal in a component: every string goes through <code>t()</code>.</li>
			</ul>
		</section>

		<section>
			<h2>Colour: primitives</h2>
			<div class="swatches">
				{#each primitives as name (name)}
					<div class="swatch"><span style:background="var(--{name})"></span><code>--{name}</code></div>
				{/each}
			</div>
		</section>

		<section>
			<h2>Colour: semantics</h2>
			<div class="swatches">
				{#each semantics as name (name)}
					<div class="swatch"><span style:background="var(--{name})"></span><code>--{name}</code></div>
				{/each}
			</div>
		</section>

		<section>
			<h2>Type</h2>
			<Stack>
				<p class="caps">Title: bold, upper case, spaced</p>
				<p class="strong">Emphasis: bold</p>
				<p>Body: regular. The quick brown fox jumps over the lazy dog. 0123456789</p>
				<p class="muted">Muted: secondary information</p>
			</Stack>
		</section>

		<section>
			<h2>Space</h2>
			<div class="spaces">
				{#each spaces as n (n)}
					<div class="space"><span style:width="var(--sp-{n})"></span><code>--sp-{n}</code></div>
				{/each}
			</div>
		</section>

		<section>
			<h2>Button</h2>
			<div class="row">
				<Button>Primary</Button>
				<Button variant="ghost">Ghost</Button>
				<Button variant="danger">Danger</Button>
				<Button href="/ds">Link</Button>
			</div>
			<div class="row">
				<Button disabled>Primary</Button>
				<Button variant="ghost" disabled>Ghost</Button>
				<Button variant="danger" disabled>Danger</Button>
			</div>
			<div class="row">
				<Button loading>Primary</Button>
				<Button variant="ghost" loading>Ghost</Button>
				<Button variant="danger" loading>Danger</Button>
				<Button variant="ghost" {loading} onclick={pretend}>Click to wait</Button>
			</div>
		</section>

		<section>
			<h2>Field and Input</h2>
			<div class="form">
				<Stack>
					<Field label="Email" for="ds-email" hint="A hint explains what is expected.">
						<Input id="ds-email" type="email" placeholder="you@example.com" bind:value={email} />
					</Field>
					<Field label="Shape" hint="Without `for`, the label names a group instead of one control.">
						<Segmented
							options={[
								{ value: 'a', label: 'Generated' },
								{ value: 'b', label: 'Earth' }
							]}
							value="a"
							label="Shape"
						/>
					</Field>
					<Field label="Code" for="ds-code" hint="One cell per digit, in two halves, as the email spells it. Paste it with the space.">
						<CodeInput id="ds-code" bind:value={code} />
					</Field>
					<Field label="Refused code" for="ds-code-refused" error="This code is wrong or expired.">
						<CodeInput id="ds-code-refused" value="12345678" invalid />
					</Field>
					<Field label="Seed" for="ds-seed" error="An error replaces the hint and is shown whole.">
						<Input id="ds-seed" value="not a seed" invalid />
					</Field>
					<Field label="Read only" for="ds-readonly">
						<Input id="ds-readonly" value="00000000deadbeef" readonly />
					</Field>
					<Field label="Disabled" for="ds-disabled">
						<Input id="ds-disabled" value="Unavailable" disabled />
					</Field>
				</Stack>
			</div>
		</section>

		<section>
			<h2>Segmented</h2>
			<div class="row">
				<Segmented options={speeds} value={speed} label="Movement" onselect={(value) => (speed = value)} />
				<span class="muted">Selected: {speed}</span>
				<Segmented options={speeds} value={speed} label="Movement, unavailable" disabled />
			</div>
		</section>

		<section>
			<h2>Language switch</h2>
			<div class="row">
				<LangSwitch />
				<LangSwitch layout="list" />
			</div>
		</section>

		<section>
			<h2>Checkbox, Slider and Select</h2>
			<div class="row">
				<Checkbox checked={shadows} onchange={(value) => (shadows = value)}>Shadows</Checkbox>
				<Checkbox checked={false} disabled>Unavailable</Checkbox>
			</div>
			<Stack>
				<Slider label="Cover" value={cover} min={0} max={1} oninput={(value) => (cover = value)} />
				<Slider
					label="Settled"
					value={settled}
					min={0}
					max={1}
					oninput={(value) => (settled = value)}
					onchange={(value) => (settledAt = value)}
				/>
				<p class="note">`onchange` fires once, when the drag ends: last let go at {settledAt.toFixed(2)}.</p>
				<Slider label="Disabled" value={0.3} min={0} max={1} disabled />
				<Select label="Tone map" options={curves} value={curve} onselect={(value) => (curve = value)} />
				<Select
					label="Cartridge"
					stacked
					hint="Stacked, a form row as Field lays one. A rule parts the games from what opens a file, which the select does not keep."
					options={games}
					value={game}
					onselect={(value) => {
						if (value !== 'file') game = value;
					}}
				/>
				<Select
					label="Colour"
					stacked
					hint="With `option`, a snippet that draws each option and the trigger, told which, a picture beside a name: a combobox then, walked by arrows, Home, End and a letter."
					options={colours}
					value={colour}
					onselect={(value) => (colour = value)}
				>
					{#snippet option(one)}
						<span class="drawn"><span class="chip" style:background="var(--{one.value})"></span>{one.label}</span>
					{/snippet}
				</Select>
			</Stack>
		</section>

		<section>
			<h2>Large panel, Tabs and Knob</h2>
			<p class="muted">
				A large Panel is a middle one of a fixed size, the whole screen on a phone. Tabs list its sections down its side, a row across the top on a
				phone. A Knob is a setting with the way back to its default, shown while it stands off it; Knobs lines a column of them up.
			</p>
			<div class="canvas canvas--tall">
				<Panel title="Settings" large>
					<div class="tabbed">
						<Tabs options={sectionTabs} value={sectionTab} label="Sections" id="ds-sections" onselect={(value) => (sectionTab = value)} />
						<div role="tabpanel" id="ds-sections-{sectionTab}">
							<Knobs>
								<Knob changed={reach !== 256} fallback="256 m" resetLabel="Back to 256 m" onreset={() => (reach = 256)}>
									<Slider label="Pictures drawn up to" value={reach} min={32} max={2048} step={32} format={(n) => `${n} m`} oninput={(value) => (reach = value)} />
								</Knob>
								<Knob changed={!shadows} fallback="On" resetLabel="Back to On" onreset={() => (shadows = true)}>
									<Checkbox checked={shadows} onchange={(value) => (shadows = value)}>Shadows</Checkbox>
								</Knob>
							</Knobs>
						</div>
					</div>
				</Panel>
			</div>
		</section>

		<section>
			<h2>Scrollbar</h2>
			<p class="muted">Every scroller in the house, from base.css: a thin rounded thumb with no track, stronger under the pointer.</p>
			<div class="scroller">
				{#each Array.from({ length: 24 }, (_, i) => i + 1) as line (line)}
					<p>Line {line}</p>
				{/each}
			</div>
		</section>

		<section>
			<h2>Icon</h2>
			<p class="muted">A stroke in the current colour at the text size. Lucide's paths, copied one at a time: the set is what the product uses.</p>
			<div class="row">
				<Icon name="users" label="Near" />
				<Icon name="globe" label="World" />
				<Icon name="send" label="Send" />
				<Icon name="message-circle" label="Chat" />
				<Icon name="map-pin" label="Place" />
				<Icon name="paw-print" label="Pet" />
				<Icon name="flashlight" label="Flashlight" />
				<Icon name="pencil" label="Edit" />
				<Icon name="log-out" label="Sign out" />
				<Icon name="layout-dashboard" label="Backoffice" />
				<Icon name="settings" label="Settings" />
				<Icon name="puzzle" label="Plugins" />
				<Icon name="languages" label="Language" />
				<Icon name="box" label="Cube" />
				<Icon name="image" label="Picture" />
				<Icon name="video" label="Video" />
				<Icon name="move-3d" label="Move" />
				<Icon name="rotate-3d" label="Rotate" />
				<Icon name="scale-3d" label="Scale" />
				<Icon name="rotate-ccw" label="Reset" />
				<Icon name="package" label="Voxels" />
				<Icon name="pyramid" label="3D model" />
				<Icon name="star" label="Landmark" />
				<Icon name="circle" label="Portal" />
				<Icon name="pause" label="Held" />
				<Icon name="gamepad-2" label="Arcade cabinet" />
				<Icon name="lock" label="Locked" />
				<Icon name="lock-open" label="Open" />
				<Icon name="list" label="List" />
				<Icon name="circle-help" label="Help" />
				<Icon name="chevrons-up" label="Jump" />
				<Icon name="chevrons-down" label="Descend" />
				<Icon name="plane" label="Fly" />
				<Icon name="eye" label="First person" />
				<Icon name="download" label="Download" />
				<Button variant="ghost"><Icon name="send" /> With a label</Button>
			</div>
			<p class="muted">IconButton: an action said by its icon alone, square, for a corner where words do not fit. The label is required; it is the accessible name and the tooltip. A switch says it is on with <code>aria-pressed</code>, as one that opens says it is open with <code>aria-expanded</code>. A <code>badge</code> carries a count; a <code>mark</code> says something new waits in what it opens, a dot read at a glance, and its label says so in words.</p>
			<div class="row">
				<IconButton icon="log-out" label="Sign out" />
				<IconButton href="/ds" icon="layout-dashboard" label="Backoffice" />
				<IconButton icon="pencil" label="Edit" disabled />
				<IconButton icon="flashlight" label="Flashlight" aria-pressed={true} />
				<IconButton icon="users" label="Here" badge="3" />
				<IconButton icon="message-circle" label="Chat, something new" mark />
			</div>
		</section>
		<section>
			<h2>Palette</h2>
			<p>The paints of a world as they will be laid: the colour in a finish and with an edge, the one in hand pressed.</p>
			<Palette colors={['#f2f0eb', '#b8b8b3', '#5c5c5a', '#1e1e1f', '#9c3b2e', '#c8693f', '#d9c08c', '#7a5230']} paint={4} finish="matte" edge="none" label="Colour" pick={(n) => `Colour ${n}`} onpick={() => {}} />
			<Palette colors={['#f2f0eb', '#b8b8b3', '#5c5c5a', '#1e1e1f', '#9c3b2e', '#c8693f', '#d9c08c', '#7a5230']} paint={1} finish="glass" edge="black" label="Colour, glass" pick={(n) => `Colour ${n}`} onpick={() => {}} />
		</section>
		<section>
			<h2>VectorInput</h2>
			<p>A vector, one field an axis: drag a field sideways to scrub its value, Shift for ten times the step, click to type.</p>
			<VectorInput label="Position" axes={['u', 'v', 'h']} values={[1024.5, 980.25, 12]} step={0.5} onchange={() => {}} />
			<VectorInput label="Rotation" axes={['x', 'y', 'z']} values={[0, 0, 45]} step={1} decimals={0} min={-180} max={180} onchange={() => {}} onreset={() => {}} resetLabel="Reset" />
			<VectorInput label="Size, locked" axes={['x', 'y', 'z']} values={[2, 2, 4]} step={0.5} onchange={() => {}} lockLabel="Keep the proportion" locked />
		</section>

		<section>
			<h2>Alert</h2>
			<Stack>
				<Alert>Information that does not block anything.</Alert>
				<Alert title="With a title">The title names the state; the body says what to do next.</Alert>
				<Alert variant="danger" title="Something failed">The message appears whole, with the way out.</Alert>
			</Stack>
		</section>

		<section>
			<h2>Badge and Spinner</h2>
			<div class="row">
				<Badge>Outline</Badge>
				<Badge variant="solid">Solid</Badge>
				<Spinner />
			</div>
		</section>

		<section>
			<h2>Over the world</h2>
			<p class="note">
				Its own family: what floats on the picture of the world, dark, translucent and soft, where the panels are light and
				square. A card always has a title and an action; a hold's verb takes a third line, marked by the ring's glyph, and
				comes forward as the ring fills. At rest the ring draws nothing; a tap sweeps it round once, fast, with a pulse; a
				hold fills it from zero once the key is held past a tap. A dial is the ring alone, a glyph sized to sit clear inside
				its inner edge, on a round chip, and a line under it. Shown over sample grounds: grass in sun, snow, night.
			</p>
			<div class="ground ground--night">
				<!-- The ring's four states: rest, a tap's sweep, a hold in progress, a hold full. -->
				<WorldCard key="E" title="Rest" action="Leave" closed={null} />
				<WorldCard key="E" title="Tap" action="Leave" closed={null} taps={sweeps} />
				<WorldCard key="E" title="Holding" action="Ride" hold="Tame" closed={0.45} />
				<WorldCard key="E" title="Held" action="Ride" hold="Tame" closed={1} />
			</div>
			{#each ['sun', 'snow', 'night'] as ground (ground)}
				<div class="ground ground--{ground}">
					<WorldCard key="E" title="Crate" action="Open" closed={null} {taps} />
					<WorldCard key="E" title="Horse" action="Ride" hold="Tame" {closed} {pulse} />
					<WorldCard key="E" title="Chest" hold="Take" {closed} {pulse} />
					<WorldCard title="Board" action="Leave" closed={null} {taps} />
					{#if ground === 'night'}
						<!-- The parts alone, which only ever float inside a card. -->
						<KeyBadge label="E" />
						<Ring {closed} {pulse}><KeyBadge /></Ring>
						<span class="glyph"><Ring {closed} /></span>
						<!-- A countdown: told its time, never eased. -->
						<Ring {closed} eased={false}><Icon name="pause" /></Ring>
						<!-- Dials over a game's screen: a countdown, and a hold. -->
						<Dial glyph="pause" {closed} eased={false} line="4 s" />
						<Dial glyph="rotate-ccw" {closed} line="reset" />
					{/if}
				</div>
			{/each}
			<Stack>
				<Slider label="Closed" value={closed ?? 0} min={0} max={1} oninput={(value) => (closed = value)} />
				<div class="row">
					<Button variant="ghost" onclick={() => (taps += 1)}>Tap</Button>
					<Button variant="ghost" onclick={() => (closed = null)}>Let go</Button>
					<Button
						variant="ghost"
						onclick={() => {
							closed = null;
							pulse += 1;
						}}>Close</Button
					>
				</div>
			</Stack>
		</section>

		<section>
			<h2>Marker and MarkerCard</h2>
			<p class="note">
				Of the family over the world: something pointed out wherever it is, a chip with a glyph, a name, a note when it has
				one, and how far. Over the thing it stands bare; at the screen's edge an arrow first in it turns toward the thing,
				degrees clockwise from up. Pressed, an outline marks it and its card opens where it was, the focus on the first
				choice. The engine places both; hidden has no look, it is not drawn.
			</p>
			{#each ['sun', 'snow', 'night'] as ground (ground)}
				<div class="ground ground--{ground}">
					<Marker glyph="paw-print" name="Bramble" distance="12 m" />
					<Marker glyph="paw-print" name="Bramble" note="on its way" distance="2.4 km" />
					<Marker glyph="house" name="Home" distance="340 m" />
					<Marker glyph="star" name="Old oak" distance="1.2 km" />
					<Marker glyph="map-pin" name="Well" distance="64 m" open />
				</div>
			{/each}
			<div class="ground ground--night">
				<!-- At the edge: the arrow at a few angles. -->
				{#each [0, 45, 90, 180, 270] as arrow (arrow)}
					<Marker glyph="star" name="{arrow}°" distance="1.2 km" {arrow} />
				{/each}
			</div>
			<div class="ground ground--sun marker-open">
				<Marker glyph="paw-print" name="Bramble" distance="12 m" open={markerOpen} onclick={() => (markerOpen = !markerOpen)} />
				{#if markerOpen}
					<MarkerCard glyph="paw-print" name="Bramble" distance="12 m" choices={markerChoices} />
				{/if}
				{#if chosen}<span class="glyph">Chose {chosen}.</span>{/if}
			</div>
		</section>

		<section>
			<h2>Dialog</h2>
			<div class="row">
				<Button variant="ghost" onclick={() => (confirming = true)}>Confirm</Button>
				<Button variant="danger" onclick={() => (deleting = true)}>Delete</Button>
				<Button variant="ghost" onclick={() => (choosing = true)}>One way</Button>
				{#if outcome}<span class="muted">{outcome}</span>{/if}
			</div>
			<Dialog
				bind:open={confirming}
				title="Make operator"
				confirmLabel="Make operator"
				onconfirm={async () => {
					await wait(800);
					outcome = 'Confirmed.';
				}}
			>
				<p>A plain confirmation. The confirm button shows the wait.</p>
			</Dialog>
			<Dialog
				bind:open={deleting}
				title="Delete world"
				confirmLabel="Delete"
				danger
				onconfirm={async () => {
					await wait(800);
					outcome = 'Deleted.';
				}}
			>
				<p>Destructive actions always confirm here. With <code>action</code> the box posts to a form action instead.</p>
			</Dialog>
			<Dialog
				bind:open={choosing}
				title="Graphics"
				confirmLabel="Start"
				oneWay
				onconfirm={() => {
					outcome = `Started ${picture}.`;
				}}
			>
				<p>One way forward, a choice always made: no cancel, and Esc or a click outside is the button.</p>
				<Segmented options={pictures} value={picture} label="Picture" onselect={(value) => (picture = value)} />
			</Dialog>
		</section>

		<section>
			<h2>Fold and Pairs</h2>
			<Fold title="About this machine">
				<Pairs
					pairs={[
						{ label: 'Browser', value: 'Google Chrome 131.0.6778.86' },
						{ label: 'WebGL2', value: 'ANGLE (AMD, AMD Radeon(TM) Graphics (0x00001638) Direct3D11 vs_5_0 ps_5_0, D3D11)' },
						{ label: 'Screen', value: '1536 × 864 px at 1.25' }
					]}
				/>
			</Fold>
			<div class="ground ground--sun">
				<Pairs onWorld pairs={[{ label: 'Frame', value: '16.7 ms, longest 33 ms' }]} />
			</div>
		</section>

		<section>
			<h2>Table and Pager</h2>
			<Table>
				<table>
					<thead>
						<tr><th>Name</th><th>Seed</th><th>Operator</th><th></th></tr>
					</thead>
					<tbody>
						<tr>
							<td>First light</td>
							<td class="muted">00000000deadbeef</td>
							<td><Badge variant="solid">Yes</Badge></td>
							<td class="actions"><Button variant="ghost">Enter</Button><Button variant="danger">Delete</Button></td>
						</tr>
						<tr>
							<td>Quiet moon</td>
							<td class="muted">a3f09c1e55d2b7c4</td>
							<td><Badge>No</Badge></td>
							<td class="actions"><Button variant="ghost">Enter</Button><Button variant="danger">Delete</Button></td>
						</tr>
					</tbody>
				</table>
			</Table>
			<Pager page={2} pages={5} />
		</section>

		<section>
			<h2>Panel and Stat</h2>
			<p class="muted">Floats in a corner of the engine canvas, or in its middle. The hatched box stands in for the canvas.</p>
			<div class="canvas">
				<Panel title="New planet">
					{#snippet aside()}<a href="/ds">planet</a>{/snippet}
					<dl>
						<Stat label="Frames" value="60" />
						<Stat label="Altitude" value="12.5 m" />
					</dl>
				</Panel>
				<Panel title="Bottom right" corner="bottom-right">
					<p>Any corner.</p>
				</Panel>
				<Panel title="Middle" corner="middle">
					<p>As wide as its content.</p>
				</Panel>
			</div>
		</section>

		<section>
			<h2>Veil</h2>
			<p class="muted">Over the picture until it is the world asked for: a wait, or what stands in the world's place.</p>
			<div class="row">
				<div class="veiled"><Veil busy word="Starting the engine" /></div>
				<div class="veiled">
					<Veil word="This world is on its way" lede="Its admin is choosing its planet. Come back soon.">
						<Button>Sign in</Button>
					</Veil>
				</div>
			</div>
		</section>

		<section>
			<h2>Topbar</h2>
			<p class="muted">The bar of what is off the planet is at the top of this page.</p>
		</section>
	</div>
</Page>

<style>
	.veiled {
		position: relative;
		flex: 1 1 280px;
		min-height: 200px;
		border: var(--bw) solid var(--border-strong);
		background: var(--bg-inset);
	}
	.sections {
		display: flex;
		flex-direction: column;
		gap: var(--sp-7);
	}
	section {
		display: flex;
		flex-direction: column;
		gap: var(--sp-4);
	}
	h2 {
		padding-bottom: var(--sp-2);
		border-bottom: var(--bw) solid var(--border);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
	.rules {
		display: flex;
		flex-direction: column;
		gap: var(--sp-2);
		max-width: var(--measure);
	}
	.rules li::before {
		content: '- ';
	}
	code {
		background: var(--bg-inset);
	}
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--sp-3);
	}
	.form {
		max-width: var(--page-narrow);
	}
	.drawn {
		display: flex;
		align-items: center;
		gap: var(--sp-3);
	}
	.chip {
		width: var(--sp-6);
		height: var(--sp-6);
		border: var(--bw) solid var(--border-strong);
	}
	.swatches {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(calc(3 * var(--sp-8)), 1fr));
		gap: var(--sp-3);
	}
	.swatch {
		display: flex;
		flex-direction: column;
		gap: var(--sp-2);
	}
	.swatch span {
		height: var(--sp-7);
		border: var(--bw) solid var(--border-strong);
	}
	.spaces {
		display: flex;
		flex-direction: column;
		gap: var(--sp-2);
	}
	.space {
		display: flex;
		align-items: center;
		gap: var(--sp-3);
	}
	.space span {
		height: var(--sp-3);
		background: var(--action);
	}
	.caps {
		font-weight: var(--fw-bold);
		letter-spacing: var(--ls-caps);
		text-transform: uppercase;
	}
	.strong {
		font-weight: var(--fw-bold);
	}
	.muted {
		color: var(--text-muted);
	}
	.canvas {
		position: relative;
		height: calc(5 * var(--sp-8));
		border: var(--bw) solid var(--border-strong);
		background: repeating-linear-gradient(45deg, var(--bg-inset) 0 var(--sp-3), var(--bg) var(--sp-3) var(--sp-5));
	}
	.scroller {
		height: calc(4 * var(--sp-8));
		overflow-y: auto;
		padding: var(--sp-3);
		border: var(--bw) solid var(--border);
	}
	.canvas--tall {
		height: calc(9 * var(--sp-8));
	}
	.tabbed {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: var(--sp-6);
		flex: 1;
		min-height: 0;
	}
	/* A large panel's content brings its scroller to the panel's edges and pads itself. */
	.tabbed > [role='tabpanel'] {
		min-height: 0;
		overflow-y: auto;
		padding: 0 calc(var(--panel-pad) - var(--scroll-w)) var(--sp-5) 0;
		scrollbar-gutter: stable;
	}
	/* Sample grounds for what floats over the world: not tokens, the world's colours. */
	.ground {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--sp-5);
		padding: var(--sp-6) var(--sp-5);
	}
	.ground--sun {
		background: linear-gradient(#8ec5f0, #b9e07a 45%, #6fae3a);
	}
	.ground--snow {
		background: linear-gradient(#dfeefa, #f7f9fb 45%, #e6ecf1);
	}
	.ground--night {
		background: linear-gradient(#05070f, #0d1424 45%, #0f1d12);
	}
	.glyph {
		color: var(--on-world);
	}
	.marker-open {
		flex-direction: column;
		align-items: flex-start;
	}
</style>
