import { t } from "$lib/i18n";

/** What a holder of the controls is pressed by, besides leaving. */
export const CONTROLS = ["stick", "jump", "sprint", "start"] as const;

export type Control = (typeof CONTROLS)[number];

/**
 * The key a touch screen's pad presses for each control but the stick, by
 * the name the web gives it: a game's own A and B, which press nothing on
 * foot, and start. The stick is the web shell's, read from the fingers.
 */
export const PAD_KEYS = {
  jump: "KeyZ",
  sprint: "KeyX",
  start: "Enter",
} as const satisfies Record<Exclude<Control, "stick">, string>;

/** The key that takes the controls back from their holder. */
export const LEAVE_KEY = "Escape";

/**
 * What `control` does, in the holder's words where it has them,
 * `control.jump` for a game's A, else in the core's.
 */
export function said(
  control: Control,
  words?: (key: string) => string,
): string {
  const key = `control.${control}`;
  const own = words?.(key);
  return own && own !== key ? own : t(`engine.controls.${control}`);
}

/**
 * What holding `control` down does, in the holder's words,
 * `control.start.hold` for a cabinet's reset: null when the holder gives
 * it nothing to do.
 */
export function held(
  control: Control,
  words?: (key: string) => string,
): string | null {
  const key = `control.${control}.hold`;
  const own = words?.(key);
  return own && own !== key ? own : null;
}
