//! Screen state and key handling. The main loop performs the returned [`Action`]s.
use dw4vhp_core::options::PatchSelection;
use dw4vhp_core::patch::PatchOutcome;
use dw4vhp_core::writer::{Phase, Progress};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Results from worker threads.
pub enum Msg {
    /// The disc label, or the notice explaining why it was refused.
    Checked(Result<String, String>),
    Progress(Progress),
    Finished(Result<PatchOutcome, String>),
}

/// Work for the main loop.
pub enum Action {
    None,
    Quit,
    Check(PathBuf),
    Patch {
        input: PathBuf,
        output: PathBuf,
        selection: PatchSelection,
        overwrite: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Disc,
    Options,
    Output,
    Patching,
    Done,
    Failed,
}

/// A question that takes over the keyboard until answered.
pub enum Prompt {
    Load(String),
    Save(String),
    Overwrite,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Opt {
    FourPlayerSpawns,
    RandomEnemies,
    RandomBosses,
    EnemySeed,
    SoloButtons,
    CrownMod,
    CrownDrops,
    RandomCrownColours,
    FixDeathSoftlock,
}

impl Opt {
    pub fn title(self) -> &'static str {
        match self {
            Opt::FourPlayerSpawns => "4-player enemy spawns",
            Opt::RandomEnemies => "Random enemies (experimental)",
            Opt::RandomBosses => "Random bosses",
            Opt::EnemySeed => "Enemy seed",
            Opt::SoloButtons => "Solo co-op buttons",
            Opt::CrownMod => "Crown Mod",
            Opt::CrownDrops => "Match drops to crowns",
            Opt::RandomCrownColours => "Random crown colours",
            Opt::FixDeathSoftlock => "Fix death softlock",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Opt::FourPlayerSpawns => "Spawn the same number of enemies as four-player mode.",
            Opt::RandomEnemies => "Different regular enemies on each difficulty, with matching stats. Protected encounters stay intact.",
            Opt::RandomBosses => "Shuffle every boss except the four world bosses.",
            Opt::EnemySeed => "The same seed on the same version gives the same game. Type digits, or press R for a new seed.",
            Opt::SoloButtons => "In single player, a pressed two-player button stays down until you press its partner.",
            Opt::CrownMod => "Stronger enemies, increased EXP rewards and crowns for every enemy.",
            Opt::CrownDrops => "Improve enemy drops for the increased difficulty.",
            Opt::RandomCrownColours => "Give every enemy and boss a random crown colour.",
            Opt::FixDeathSoftlock => "Dying before your first death tutorial has played (for example after entering Goblin Fortress early) no longer freezes the game.",
        }
    }

    /// Sub-options are indented under their parent.
    pub fn nested(self) -> bool {
        matches!(self, Opt::RandomBosses | Opt::EnemySeed | Opt::CrownDrops)
    }
}

pub enum Row {
    Header(&'static str),
    Item(Opt),
}

/// The largest seed the desktop app accepts.
const SEED_DIGITS: usize = 10;

pub struct App {
    pub screen: Screen,
    pub disc_field: String,
    /// The checked disc and its label.
    pub disc: Option<(PathBuf, String)>,
    pub checking: bool,
    pub selection: PatchSelection,
    pub seed_text: String,
    pub cursor: usize,
    pub output_field: String,
    /// The output name last suggested, replaced on return unless the player edited it.
    suggested: String,
    pub prompt: Option<Prompt>,
    /// A one-line message for the current screen; `true` marks an error.
    pub notice: Option<(String, bool)>,
    pub progress: Option<Progress>,
    pub started: Instant,
    pub outcome: Option<(PathBuf, PatchOutcome, Option<u32>)>,
    pub failure: String,
    pending_output: PathBuf,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Disc,
            disc_field: String::new(),
            disc: None,
            checking: false,
            selection: PatchSelection::default(),
            seed_text: String::new(),
            cursor: 0,
            output_field: String::new(),
            suggested: String::new(),
            prompt: None,
            notice: None,
            progress: None,
            started: Instant::now(),
            outcome: None,
            failure: String::new(),
            pending_output: PathBuf::new(),
        }
    }

    /// Starts checking `path` as if it had been typed on the disc screen.
    pub fn open(&mut self, path: &Path) -> Action {
        self.disc_field = path.display().to_string();
        self.check()
    }

    pub fn rows(&self) -> Vec<Row> {
        let s = &self.selection;
        let mut rows = vec![Row::Header("Gameplay"), Row::Item(Opt::FourPlayerSpawns)];
        rows.push(Row::Item(Opt::RandomEnemies));
        if s.random_enemies {
            rows.extend([Row::Item(Opt::RandomBosses), Row::Item(Opt::EnemySeed)]);
        }
        rows.extend([
            Row::Item(Opt::SoloButtons),
            Row::Header("Balance"),
            Row::Item(Opt::CrownMod),
        ]);
        if s.crown_mod {
            rows.push(Row::Item(Opt::CrownDrops));
        }
        rows.extend([
            Row::Header("Cosmetics"),
            Row::Item(Opt::RandomCrownColours),
            Row::Header("Bugfixes"),
            Row::Item(Opt::FixDeathSoftlock),
        ]);
        rows
    }

    pub fn items(&self) -> Vec<Opt> {
        self.rows()
            .into_iter()
            .filter_map(|row| match row {
                Row::Item(opt) => Some(opt),
                Row::Header(_) => None,
            })
            .collect()
    }

    pub fn current(&self) -> Opt {
        let items = self.items();
        items[self.cursor.min(items.len() - 1)]
    }

    pub fn enabled(&self, opt: Opt) -> bool {
        let s = &self.selection;
        match opt {
            Opt::FourPlayerSpawns => s.four_player_spawns,
            Opt::RandomEnemies => s.random_enemies,
            Opt::RandomBosses => s.random_bosses,
            Opt::EnemySeed => true,
            Opt::SoloButtons => s.solo_buttons,
            Opt::CrownMod => s.crown_mod,
            Opt::CrownDrops => s.crown_drops,
            Opt::RandomCrownColours => s.random_crown_colours,
            Opt::FixDeathSoftlock => s.fix_death_softlock,
        }
    }

    fn toggle(&mut self, opt: Opt) {
        let s = &mut self.selection;
        match opt {
            Opt::FourPlayerSpawns => s.four_player_spawns ^= true,
            Opt::RandomEnemies => {
                s.random_enemies ^= true;
                if !s.random_enemies {
                    s.random_bosses = false;
                } else if parse_seed(&self.seed_text).is_none() {
                    self.seed_text = new_seed();
                }
            }
            Opt::RandomBosses => s.random_bosses ^= true,
            Opt::EnemySeed => {}
            Opt::SoloButtons => s.solo_buttons ^= true,
            Opt::CrownMod => {
                s.crown_mod ^= true;
                if !s.crown_mod {
                    s.crown_drops = false;
                }
            }
            Opt::CrownDrops => s.crown_drops ^= true,
            Opt::RandomCrownColours => s.random_crown_colours ^= true,
            Opt::FixDeathSoftlock => s.fix_death_softlock ^= true,
        }
        self.cursor = self.cursor.min(self.items().len() - 1);
    }

    /// The selection the patch will use, with the typed seed when enemies are random.
    pub fn patch_selection(&self) -> PatchSelection {
        PatchSelection {
            enemy_seed: parse_seed(&self.seed_text).filter(|_| self.selection.random_enemies),
            ..self.selection
        }
    }

    fn settings_path(&self) -> String {
        let dir = self
            .disc
            .as_ref()
            .and_then(|(path, _)| path.parent())
            .map(Path::to_path_buf)
            .unwrap_or_default();
        dir.join("crown-mod-settings.json").display().to_string()
    }

    pub fn receive(&mut self, msg: Msg) {
        match msg {
            Msg::Checked(result) => {
                self.checking = false;
                match result {
                    Ok(label) => {
                        let path = PathBuf::from(clean_path(&self.disc_field));
                        self.disc = Some((path, label));
                        self.notice = None;
                        self.screen = Screen::Options;
                    }
                    Err(notice) => self.notice = Some((notice, true)),
                }
            }
            Msg::Progress(progress) => self.progress = Some(progress),
            Msg::Finished(Ok(outcome)) => {
                let seed = self.patch_selection().enemy_seed;
                self.outcome = Some((std::mem::take(&mut self.pending_output), outcome, seed));
                self.screen = Screen::Done;
            }
            Msg::Finished(Err(error)) => {
                self.failure = error;
                self.screen = Screen::Failed;
            }
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') && self.screen != Screen::Patching {
            return Action::Quit;
        }
        if let Some(prompt) = self.prompt.take() {
            return self.prompt_key(prompt, key);
        }
        match self.screen {
            Screen::Disc => self.disc_key(key),
            Screen::Options => self.options_key(key),
            Screen::Output => self.output_key(key),
            Screen::Patching => Action::None,
            Screen::Done => match key.code {
                KeyCode::Enter | KeyCode::Esc => Action::Quit,
                _ => Action::None,
            },
            Screen::Failed => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                    self.screen = Screen::Options;
                }
                Action::None
            }
        }
    }

    fn check(&mut self) -> Action {
        let path = clean_path(&self.disc_field);
        if path.is_empty() {
            self.notice = Some(("Enter the path to your disc image.".into(), true));
            return Action::None;
        }
        self.checking = true;
        self.notice = None;
        Action::Check(PathBuf::from(path))
    }

    fn disc_key(&mut self, key: KeyEvent) -> Action {
        if self.checking {
            return Action::None;
        }
        match key.code {
            KeyCode::Enter => self.check(),
            KeyCode::Esc => Action::Quit,
            _ => {
                edit(&mut self.disc_field, key);
                Action::None
            }
        }
    }

    fn options_key(&mut self, key: KeyEvent) -> Action {
        let items = self.items().len();
        let current = self.current();
        self.notice = None;
        match key.code {
            KeyCode::Up => self.cursor = self.cursor.checked_sub(1).unwrap_or(items - 1),
            KeyCode::Down | KeyCode::Tab => self.cursor = (self.cursor + 1) % items,
            KeyCode::Char(' ') => self.toggle(current),
            KeyCode::Esc => {
                self.screen = Screen::Disc;
            }
            KeyCode::Enter => self.continue_to_output(),
            KeyCode::Char(c) if current == Opt::EnemySeed && c.is_ascii_digit() => {
                if self.seed_text.len() < SEED_DIGITS {
                    self.seed_text.push(c);
                }
            }
            KeyCode::Backspace if current == Opt::EnemySeed => {
                self.seed_text.pop();
            }
            KeyCode::Char('r' | 'R') if current == Opt::EnemySeed => self.seed_text = new_seed(),
            KeyCode::Char('l' | 'L') => self.prompt = Some(Prompt::Load(self.settings_path())),
            KeyCode::Char('s' | 'S') => self.prompt = Some(Prompt::Save(self.settings_path())),
            _ => {}
        }
        Action::None
    }

    fn continue_to_output(&mut self) {
        let selection = self.patch_selection();
        if selection.random_enemies && selection.enemy_seed.is_none() {
            self.notice = Some(("Enter an enemy seed from 0 to 4294967295.".into(), true));
            return;
        }
        if let Err(error) = selection.into_plan() {
            self.notice = Some((error.into(), true));
            return;
        }
        let Some((input, _)) = &self.disc else {
            return;
        };
        let suggestion = dw4vhp_core::naming::default_output(input, selection.enemy_seed)
            .display()
            .to_string();
        if self.output_field.is_empty() || self.output_field == self.suggested {
            self.output_field = suggestion.clone();
        }
        self.suggested = suggestion;
        self.screen = Screen::Output;
    }

    fn output_key(&mut self, key: KeyEvent) -> Action {
        self.notice = None;
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::Options;
                Action::None
            }
            KeyCode::Enter => {
                let output = PathBuf::from(clean_path(&self.output_field));
                if output.as_os_str().is_empty() {
                    self.notice = Some(("Enter where to save the patched game.".into(), true));
                    return Action::None;
                }
                match output.try_exists() {
                    Ok(true) => {
                        self.prompt = Some(Prompt::Overwrite);
                        Action::None
                    }
                    Ok(false) => self.start(false),
                    Err(error) => {
                        self.notice = Some((error.to_string(), true));
                        Action::None
                    }
                }
            }
            _ => {
                edit(&mut self.output_field, key);
                Action::None
            }
        }
    }

    fn start(&mut self, overwrite: bool) -> Action {
        let Some((input, _)) = &self.disc else {
            return Action::None;
        };
        let output = PathBuf::from(clean_path(&self.output_field));
        self.pending_output = output.clone();
        self.progress = None;
        self.started = Instant::now();
        self.screen = Screen::Patching;
        Action::Patch {
            input: input.clone(),
            output,
            selection: self.patch_selection(),
            overwrite,
        }
    }

    fn prompt_key(&mut self, prompt: Prompt, key: KeyEvent) -> Action {
        match (prompt, key.code) {
            (Prompt::Overwrite, KeyCode::Char('y' | 'Y')) => return self.start(true),
            (Prompt::Overwrite, KeyCode::Char('n' | 'N') | KeyCode::Esc) => {}
            (Prompt::Overwrite, _) => self.prompt = Some(Prompt::Overwrite),
            (Prompt::Load(_) | Prompt::Save(_), KeyCode::Esc) => {}
            (Prompt::Load(field), KeyCode::Enter) => self.load(&field),
            (Prompt::Save(field), KeyCode::Enter) => self.save(&field),
            (Prompt::Load(mut field), _) => {
                edit(&mut field, key);
                self.prompt = Some(Prompt::Load(field));
            }
            (Prompt::Save(mut field), _) => {
                edit(&mut field, key);
                self.prompt = Some(Prompt::Save(field));
            }
        }
        Action::None
    }

    fn load(&mut self, field: &str) {
        match dw4vhp_core::settings::read(Path::new(&clean_path(field))) {
            Ok(selection) => {
                self.selection = PatchSelection {
                    enemy_seed: None,
                    ..selection
                };
                self.seed_text = selection.enemy_seed.map(|s| s.to_string()).unwrap_or_default();
                self.cursor = self.cursor.min(self.items().len() - 1);
                self.notice = Some(("Settings loaded.".into(), false));
            }
            Err(error) => self.notice = Some((error, true)),
        }
    }

    fn save(&mut self, field: &str) {
        let path = PathBuf::from(clean_path(field));
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            self.notice = Some(("Save settings with a .json extension.".into(), true));
            return;
        }
        let result = dw4vhp_core::settings::encode(self.patch_selection()).and_then(|json| {
            std::fs::write(&path, json).map_err(|_| "Unable to save the settings file.".to_string())
        });
        self.notice = Some(match result {
            Ok(()) => (format!("Settings saved to {}", path.display()), false),
            Err(error) => (error, true),
        });
    }
}

/// Typing, Backspace and Ctrl+U (clear) for a text field.
fn edit(field: &mut String, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    // AltGr arrives as Ctrl+Alt on Windows and still types a character.
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char('u') if ctrl && !alt => field.clear(),
        KeyCode::Char(c) if !ctrl || alt => field.push(c),
        KeyCode::Backspace => {
            field.pop();
        }
        _ => {}
    }
}

/// Paths dropped onto a console window arrive quoted when they contain spaces.
pub fn clean_path(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text)
        .to_string()
}

fn parse_seed(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || text.len() > SEED_DIGITS || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn new_seed() -> String {
    getrandom::u32().unwrap_or_default().to_string()
}

/// The step under way, matching the desktop app's wording.
pub fn phase_label(phase: Phase) -> String {
    match phase {
        Phase::Inspecting => "Checking your disc...".into(),
        Phase::Unpacking => "Unpacking the disc image...".into(),
        Phase::Restoring => "Restoring the original game files...".into(),
        Phase::Building => "Preparing game files...".into(),
        Phase::RandomisingEnemies { area, areas } => {
            format!("Randomising enemies: area {area} of {areas}...")
        }
        Phase::RandomisingBosses { difficulty } => format!(
            "Randomising bosses: {}...",
            match difficulty {
                0 => "Normal",
                1 => "Hard",
                _ => "Very Hard",
            }
        ),
        Phase::Recording => "Saving update data...".into(),
        Phase::Copying => "Copying game files...".into(),
        Phase::Writing | Phase::Extending => "Writing your new game...".into(),
        Phase::Packing => "Compressing the disc image...".into(),
        Phase::Verifying | Phase::Hashing => "Verifying your new game...".into(),
    }
}
