/** One inspection verdict, mirroring the old StatusKind mapping. */
export interface DiscStatus {
  kind: "ok" | "refused" | "unknown";
  text: string;
  size: number;
  outputSize: number;
  copies: [number, number, number] | null;
}
