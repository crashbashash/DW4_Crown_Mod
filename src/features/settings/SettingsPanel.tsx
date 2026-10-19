import { useId } from "react";
import { Switch } from "react-aria-components";

/** Optional patches start disabled. */
export function SettingsPanel({ fourPlayerSpawns, onFourPlayerSpawnsChange, crownDrops, onCrownDropsChange, randomCrownColours, onRandomCrownColoursChange, busy }: {
  fourPlayerSpawns: boolean;
  onFourPlayerSpawnsChange: (selected: boolean) => void;
  crownDrops: boolean;
  onCrownDropsChange: (selected: boolean) => void;
  randomCrownColours: boolean;
  onRandomCrownColoursChange: (selected: boolean) => void;
  busy: boolean;
}) {
  const id = useId();
  return (
    <section className="panel settings-panel" aria-labelledby={`${id}-settings-heading`}>
      <h2 id={`${id}-settings-heading`}>Settings</h2>
      <Switch
        className="setting-row"
        isSelected={fourPlayerSpawns}
        onChange={onFourPlayerSpawnsChange}
        isDisabled={busy}
        aria-labelledby={`${id}-four-player-spawns-label`}
        aria-describedby={`${id}-four-player-spawns-description`}
      >
        <span className="setting-copy">
          <span className="setting-title" id={`${id}-four-player-spawns-label`}>4-player enemy spawns</span>
          <span className="setting-description" id={`${id}-four-player-spawns-description`}>Spawn the same number of enemies as four-player mode.</span>
        </span>
        <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
      </Switch>
      <Switch
        className="setting-row"
        isSelected={crownDrops}
        onChange={onCrownDropsChange}
        isDisabled={busy}
        aria-labelledby={`${id}-crown-drops-label`}
        aria-describedby={`${id}-crown-drops-description`}
      >
        <span className="setting-copy">
          <span className="setting-title" id={`${id}-crown-drops-label`}>Match drops to crowns</span>
          <span className="setting-description" id={`${id}-crown-drops-description`}>Improve enemy drops for the increased difficulty.</span>
        </span>
        <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
      </Switch>
      <Switch
        className="setting-row"
        isSelected={randomCrownColours}
        onChange={onRandomCrownColoursChange}
        isDisabled={busy}
        aria-labelledby={`${id}-random-crown-colours-label`}
        aria-describedby={`${id}-random-crown-colours-description`}
      >
        <span className="setting-copy">
          <span className="setting-title" id={`${id}-random-crown-colours-label`}>Random crown colours</span>
          <span className="setting-description" id={`${id}-random-crown-colours-description`}>Randomise enemy and river turret crown colours. Bosses keep yellow.</span>
        </span>
        <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
      </Switch>
    </section>
  );
}
