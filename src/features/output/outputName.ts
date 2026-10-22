/** The marker the app puts in a suggested output name, optionally followed by the enemy seed. */
const SUGGESTED = / \[CrownMod\]( \[Seed \d+\])?(\.[^./\\]+)$/;

/** Whether `path` still has the name the app suggested, so it may be adjusted without overriding the user. */
export function isSuggestedName(path: string): boolean {
  return SUGGESTED.test(path);
}

/**
 * A suggested output path with the enemy seed in brackets after the mod marker
 * (`game [CrownMod] [Seed 1234].iso`), or without one when `seed` is `null`.
 * Any other path is returned unchanged.
 */
export function withSeed(path: string, seed: number | null): string {
  return path.replace(SUGGESTED, (_, _old, extension: string) => ` [CrownMod]${seed === null ? "" : ` [Seed ${seed}]`}${extension}`);
}
