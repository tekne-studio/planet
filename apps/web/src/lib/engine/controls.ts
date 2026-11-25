import { t } from "$lib/i18n";

/** What a holder of the controls is pressed by, besides leaving. */
export const CONTROLS = ["stick", "jump", "sprint", "start"] as const;

export type Control = (typeof CONTROLS)[number];

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
