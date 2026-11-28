import type { Now } from './facts';
import type { Held } from './index';

/**
 * What changes as the world runs, read only while a list of facts is shown:
 * the frame's interval each frame, and a few times a second, never a frame,
 * what the engine holds and who has the keys. A key the engine still holds
 * after it came up shows here.
 */
export class Live {
	now = $state<Now>();

	/** Reads until the returned stop. */
	watch(held: () => Held | undefined): () => void {
		const intervals: number[] = [];
		let frame: Now['frame'] = null;
		let last = performance.now();
		let request = requestAnimationFrame(function tick(at) {
			intervals.push(at - last);
			last = at;
			if (intervals.length > 60) intervals.shift();
			if (intervals.length % 10 === 0) {
				frame = { mean: intervals.reduce((sum, ms) => sum + ms, 0) / intervals.length, longest: Math.max(...intervals) };
			}
			request = requestAnimationFrame(tick);
		});
		const read = () =>
			(this.now = { frame, held: held(), points: navigator.maxTouchPoints, hidden: document.hidden, focused: document.hasFocus() });
		read();
		const every = setInterval(read, 250);
		return () => {
			cancelAnimationFrame(request);
			clearInterval(every);
		};
	}
}
