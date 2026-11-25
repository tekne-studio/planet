/**
 * The layout a cabinet's skin is painted on, saved as a PNG at the skin's
 * own size: the picture the engine draws from the table that lays a skin on
 * the body, each of its places named here in the reader's words; and a
 * game's cover, the reference beside it.
 */
import type { CabinetTemplate, CartridgeCover, TemplateLabel } from '$lib/engine';
import type { Seam } from '$lib/plugins/plugin';
import { t } from './words';

/** The size a label's word starts at, and the smallest it shrinks to, to fit across its place, pixels. */
const WORD_PX = 13;
const SMALLEST_PX = 8;
/** Room kept clear on each side of a word, pixels. */
const MARGIN_PX = 4;

const named = (label: TemplateLabel) => t(`cabinet.template.${label.name}` as Parameters<typeof t>[0]);

/** The template drawn on a canvas, its places named. */
export function draw({ image, labels }: CabinetTemplate): HTMLCanvasElement {
	const canvas = document.createElement('canvas');
	canvas.width = image.width;
	canvas.height = image.height;
	const context = canvas.getContext('2d');
	if (!context) throw new Error('canvas');
	context.putImageData(image, 0, 0);
	const family = getComputedStyle(document.body).fontFamily || 'sans-serif';
	const font = (px: number) => `600 ${px}px ${family}`;
	context.textAlign = 'center';
	context.textBaseline = 'middle';
	for (const label of labels) {
		const [x0, y0, x1, y1] = label.rect;
		const word = named(label);
		const across = x1 - x0 - 2 * MARGIN_PX;
		// As large as the place is tall, shrunk to fit across it; past the
		// smallest, the canvas narrows the word.
		let px = Math.max(SMALLEST_PX, Math.min(WORD_PX, y1 - y0 - MARGIN_PX));
		context.font = font(px);
		while (px > SMALLEST_PX && context.measureText(word).width > across) context.font = font(--px);
		context.fillStyle = `rgb(${label.ink.join(' ')})`;
		context.fillText(word, (x0 + x1) / 2, (y0 + y1) / 2, across);
	}
	return canvas;
}

/** Saves `canvas` as the PNG named `name`, the browser's download. */
async function save(canvas: HTMLCanvasElement, name: string): Promise<boolean> {
	const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, 'image/png'));
	if (!blob) return false;
	const link = document.createElement('a');
	link.href = URL.createObjectURL(blob);
	link.download = name;
	link.click();
	setTimeout(() => URL.revokeObjectURL(link.href), 0);
	return true;
}

/** Saves the template as a PNG, the browser's download. False with no engine built to draw it. */
export async function saveTemplate(seam: Seam): Promise<boolean> {
	const template = await seam.cabinetTemplate();
	if (!template) return false;
	return save(draw(template), t('cabinet.template.file'));
}

/**
 * Saves a game's cover as a PNG at its own pixels, 160 by 120: beside the
 * template, the reference a painter takes a skin's colours and art from.
 */
export function saveCover(cover: CartridgeCover, title: string): Promise<boolean> {
	const canvas = document.createElement('canvas');
	canvas.width = cover.picture.width;
	canvas.height = cover.picture.height;
	canvas.getContext('2d')?.putImageData(cover.picture, 0, 0);
	return save(canvas, t('cabinet.cover.file', { title }));
}
