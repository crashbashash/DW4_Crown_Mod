export interface UpdateOffer {
  version: string;
  currentVersion: string;
  releases: Array<{ version: string; notes: string }>;
  method: "msi" | "browser";
}
export interface UpdateCheck {
  currentVersion: string;
  update: UpdateOffer | null;
}
