import { useId, type ReactNode } from "react";
import { Button, Switch, Tab, TabList, TabPanel, Tabs } from "react-aria-components";
import { newSeed, parseSeed } from "./seed";

function Setting({ id, title, description, selected, onChange, disabled, badge }: {
  id: string;
  title: string;
  description: ReactNode;
  selected: boolean;
  onChange: (selected: boolean) => void;
  disabled: boolean;
  badge?: string;
}) {
  return (
    <Switch className="setting-row" isSelected={selected} onChange={onChange} isDisabled={disabled}
      aria-labelledby={`${id}-label`} aria-describedby={`${id}-description`}>
      <span className="setting-copy">
        {badge === undefined
          ? <span className="setting-title" id={`${id}-label`}>{title}</span>
          : (
            <span className="setting-title-row">
              <span className="setting-title" id={`${id}-label`}>{title}</span>
              <span className="badge-experimental">{badge}</span>
            </span>
          )}
        <span className="setting-description" id={`${id}-description`}>{description}</span>
      </span>
      <span className="switch-track" aria-hidden="true"><span className="switch-thumb" /></span>
    </Switch>
  );
}

/** Optional patches start disabled. */
export function SettingsPanel({ crownMod, onCrownModChange, fourPlayerSpawns, onFourPlayerSpawnsChange, crownDrops, onCrownDropsChange, randomCrownColours, onRandomCrownColoursChange, randomEnemies, onRandomEnemiesChange, randomBosses, onRandomBossesChange, enemySeed, onEnemySeedChange, soloButtons, onSoloButtonsChange, fixDeathSoftlock, onFixDeathSoftlockChange, busy, fileActions }: {
  fileActions?: ReactNode;
  crownMod: boolean;
  onCrownModChange: (selected: boolean) => void;
  fourPlayerSpawns: boolean;
  onFourPlayerSpawnsChange: (selected: boolean) => void;
  crownDrops: boolean;
  onCrownDropsChange: (selected: boolean) => void;
  randomCrownColours: boolean;
  onRandomCrownColoursChange: (selected: boolean) => void;
  randomEnemies: boolean;
  onRandomEnemiesChange: (selected: boolean) => void;
  randomBosses: boolean;
  onRandomBossesChange: (selected: boolean) => void;
  /** The seed field's text; a valid seed is filled in when the option is switched on. */
  enemySeed: string;
  onEnemySeedChange: (text: string) => void;
  soloButtons: boolean;
  onSoloButtonsChange: (selected: boolean) => void;
  fixDeathSoftlock: boolean;
  onFixDeathSoftlockChange: (selected: boolean) => void;
  busy: boolean;
}) {
  const id = useId();
  const seedValid = parseSeed(enemySeed) !== null;
  return (
    <section className="panel settings-panel" aria-labelledby={`${id}-settings-heading`}>
      <h2 id={`${id}-settings-heading`}>Settings</h2>
      {fileActions}
      <Tabs className="settings-tabs">
        <TabList className="settings-tab-list" aria-label="Settings categories">
          <Tab id="gameplay" className="settings-tab">Gameplay</Tab>
          <Tab id="balance" className="settings-tab">Balance</Tab>
          <Tab id="cosmetics" className="settings-tab">Cosmetics</Tab>
          <Tab id="bugfixes" className="settings-tab">Bugfixes</Tab>
        </TabList>
        <TabPanel id="gameplay" className="settings-tab-panel">
          <Setting id={`${id}-four-player-spawns`} title="4-player enemy spawns" selected={fourPlayerSpawns}
            onChange={onFourPlayerSpawnsChange} disabled={busy}
            description="Spawn the same number of enemies as four-player mode." />
          <Setting id={`${id}-random-enemies`} title="Random enemies" badge="Experimental" selected={randomEnemies}
            onChange={onRandomEnemiesChange} disabled={busy}
            description="Different regular enemies on each difficulty, with matching stats. Protected encounters stay intact." />
          {randomEnemies && (
            <div className="setting-submenu" role="group" aria-label="Random enemies options">
              <Setting id={`${id}-random-bosses`} title="Random bosses" selected={randomBosses}
                onChange={onRandomBossesChange} disabled={busy}
                description="Shuffle every boss except the four world bosses." />
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
          <Setting id={`${id}-solo-buttons`} title="Solo co-op buttons" selected={soloButtons}
            onChange={onSoloButtonsChange} disabled={busy}
            description="In single player, a pressed two-player button stays down until you press its partner." />
        </TabPanel>
        <TabPanel id="balance" className="settings-tab-panel">
          <Setting id={`${id}-crown-mod`} title="Crown Mod" selected={crownMod} onChange={onCrownModChange} disabled={busy}
            description="Stronger enemies, increased EXP rewards and crowns for every enemy." />
          {crownMod && (
            <div className="setting-submenu" role="group" aria-label="Crown Mod options">
              <Setting id={`${id}-crown-drops`} title="Match drops to crowns" selected={crownDrops}
                onChange={onCrownDropsChange} disabled={busy}
                description="Improve enemy drops for the increased difficulty." />
            </div>
          )}
        </TabPanel>
        <TabPanel id="cosmetics" className="settings-tab-panel">
          <Setting id={`${id}-random-crown-colours`} title="Random crown colours" selected={randomCrownColours}
            onChange={onRandomCrownColoursChange} disabled={busy}
            description="Give every enemy and boss a random crown colour." />
        </TabPanel>
        <TabPanel id="bugfixes" className="settings-tab-panel">
          <Setting id={`${id}-fix-death-softlock`} title="Fix death softlock" selected={fixDeathSoftlock}
            onChange={onFixDeathSoftlockChange} disabled={busy}
            description="Dying before your first death tutorial has played (for example after entering Goblin Fortress early) no longer freezes the game." />
        </TabPanel>
      </Tabs>
    </section>
  );
}
