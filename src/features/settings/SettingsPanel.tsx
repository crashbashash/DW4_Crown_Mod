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

  return (
    <section className="panel settings-panel" aria-labelledby="settings-heading">
      <h2 id="settings-heading">Settings</h2>
      <Switch
        className="setting-row"
        isSelected={fourPlayerSpawns}
        onChange={onFourPlayerSpawnsChange}
        isDisabled={busy}
        aria-labelledby="four-player-spawns-label"
        aria-describedby="four-player-spawns-description"
      >
        <span className="setting-copy">
          <span className="setting-title" id="four-player-spawns-label">4-player enemy spawns</span>
          <span className="setting-description" id="four-player-spawns-description">Spawn the same number of enemies as four-player mode.</span>
        </span>
        <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
      </Switch>
      <Switch
        className="setting-row"
        isSelected={crownDrops}
        onChange={onCrownDropsChange}
        isDisabled={busy}
        aria-labelledby="crown-drops-label"
        aria-describedby="crown-drops-description"
      >
        <span className="setting-copy">
          <span className="setting-title" id="crown-drops-label">Match drops to crowns</span>
          <span className="setting-description" id="crown-drops-description">Improve enemy drops for the increased difficulty.</span>
        </span>
        <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
      </Switch>
      <Switch
        className="setting-row"
        isSelected={randomCrownColours}
        onChange={onRandomCrownColoursChange}
        isDisabled={busy}
        aria-labelledby="random-crown-colours-label"
        aria-describedby="random-crown-colours-description"
      >
        <span className="setting-copy">
          <span className="setting-title" id="random-crown-colours-label">Random crown colours</span>
          <span className="setting-description" id="random-crown-colours-description">Randomise the crown colour of each enemy spawn. Bosses keep yellow.</span>
        </span>
        <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
      </Switch>
    </section>
  );
}
