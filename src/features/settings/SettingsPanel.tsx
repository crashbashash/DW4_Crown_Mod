import { useId } from "react";
import { Button, Switch } from "react-aria-components";
import { newSeed, parseSeed } from "./seed";

/** Optional patches start disabled. */
export function SettingsPanel({ fourPlayerSpawns, onFourPlayerSpawnsChange, crownDrops, onCrownDropsChange, randomCrownColours, onRandomCrownColoursChange, randomEnemies, onRandomEnemiesChange, enemySeed, onEnemySeedChange, busy }: {
  fourPlayerSpawns: boolean;
  onFourPlayerSpawnsChange: (selected: boolean) => void;
  crownDrops: boolean;
  onCrownDropsChange: (selected: boolean) => void;
  randomCrownColours: boolean;
  onRandomCrownColoursChange: (selected: boolean) => void;
  randomEnemies: boolean;
  onRandomEnemiesChange: (selected: boolean) => void;
  /** The seed field's text; a valid seed is filled in when the option is switched on. */
  enemySeed: string;
  onEnemySeedChange: (text: string) => void;
  busy: boolean;
}) {
  const id = useId();
  const seedValid = parseSeed(enemySeed) !== null;
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
      <Switch
        className="setting-row"
        isSelected={randomEnemies}
        onChange={onRandomEnemiesChange}
        isDisabled={busy}
        aria-labelledby={`${id}-random-enemies-label`}
        aria-describedby={`${id}-random-enemies-description`}
      >
        <span className="setting-copy">
          <span className="setting-title-row">
            <span className="setting-title" id={`${id}-random-enemies-label`}>Random enemies</span>
            <span className="badge-experimental">Experimental</span>
          </span>
          <span className="setting-description" id={`${id}-random-enemies-description`}>
            Swap the regular enemies of every stage for other enemies, with matching stats. Bosses stay put.
          </span>
        </span>
        <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
      </Switch>
      {randomEnemies && (
        <div className="setting-submenu" role="group" aria-label="Random enemies options">
          <label className="setting-field" htmlFor={`${id}-enemy-seed`}>
            <span className="setting-title">Seed</span>
            <span className="setting-description">The same seed on the same version gives the same game.</span>
          </label>
          <div className="row">
            <input
              id={`${id}-enemy-seed`}
              className="path-field seed-field"
              type="text"
              inputMode="numeric"
              autoComplete="off"
              spellCheck={false}
              value={enemySeed}
              disabled={busy}
              aria-label="Enemy seed"
              aria-invalid={!seedValid}
              aria-describedby={seedValid ? undefined : `${id}-enemy-seed-error`}
              onChange={(e) => onEnemySeedChange(e.target.value)}
            />
            <Button isDisabled={busy} onPress={() => onEnemySeedChange(newSeed())}>New seed</Button>
          </div>
          {!seedValid && (
            <p className="field-error" id={`${id}-enemy-seed-error`} role="alert">
              Enter a whole number from 0 to 4294967295, or choose New seed.
            </p>
          )}
        </div>
      )}
    </section>
  );
}
