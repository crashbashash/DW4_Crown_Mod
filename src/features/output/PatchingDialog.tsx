import { ActivityDialog } from "../../components/ActivityDialog";

export function PatchingDialog({ progress }: { progress: number }) {
  return <ActivityDialog title="Patching your game" message="Creating your Crown Mod game. This may take a few minutes." value={progress} />;
}
