import { ActivityDialog } from "../../components/ActivityDialog";

export function PatchingDialog({ progress, status, updating = false }: {
  progress: number;
  status?: string;
  /** Replacing a modded image in place, rather than creating a new one. */
  updating?: boolean;
}) {
  return updating
    ? <ActivityDialog title="Updating your game" message="Rebuilding your Crown Mod game with the selected options. This may take a few minutes." value={progress} detail={status} />
    : <ActivityDialog title="Patching your game" message="Creating your Crown Mod game. This may take a few minutes." value={progress} detail={status} />;
}
