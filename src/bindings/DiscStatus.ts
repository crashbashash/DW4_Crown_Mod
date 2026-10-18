/** One inspection verdict, mirroring the old StatusKind mapping. */
export interface DiscStatus {
  kind: "ok" | "refused" | "unknown";
  text: string;
  reason?: "already-modded";
  size: number;
  outputSize: number;
  copies: [number, number, number] | null;
}
