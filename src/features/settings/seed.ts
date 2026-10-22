/** Enemy seeds are unsigned 32-bit integers, shown and typed in decimal. */
export const MAX_SEED = 0xffff_ffff;

export function newSeed(): string {
  const word = new Uint32Array(1);
  crypto.getRandomValues(word);
  return String(word[0]);
}

/** The seed a field's text names, or `null` when it is not a whole number in range. */
export function parseSeed(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d{1,10}$/.test(trimmed)) return null;
  const value = Number(trimmed);
  return value <= MAX_SEED ? value : null;
}
