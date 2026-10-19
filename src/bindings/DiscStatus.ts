/** One inspection verdict, mirroring the old StatusKind mapping. */
export interface DiscStatus {
  kind: "ok" | "refused" | "unknown";
  text: string;
  reason?: "already-modded";
  discInfo?: {
    console: "PS2" | "GameCube";
    region: "USA" | "Europe" | "Japan";
    modded: boolean;
    modVersion: string | null;
  };
  modMetadata?: {
    version: string;
    options: { fourPlayerSpawns: boolean; betterLoot: boolean; randomCrownColours: boolean };
    recovery?: { formatVersion: number };
  };
  size: number;
  outputSize: number;
  copies: [number, number, number] | null;
}
