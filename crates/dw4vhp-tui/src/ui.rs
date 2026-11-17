//! Drawing. Windows 7's console only renders 16 colours, ignores text attributes
//! and its raster font may lack box-drawing glyphs, so everything here uses ASCII
//! and named colours only.
use crate::app::{phase_label, App, Prompt, Row, Screen};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

const ASCII: border::Set = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

const ACCENT: Color = Color::Yellow;
const MUTED: Color = Color::Gray;
const ERROR: Color = Color::LightRed;
const OK: Color = Color::LightGreen;

pub fn draw(frame: &mut Frame, app: &App) {
    let [main, footer] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ASCII)
        .title(format!(" DW4 Crown Mod {} ", env!("CARGO_PKG_VERSION")))
        .title_style(Style::default().fg(ACCENT));
    let inner = block.inner(main);
    frame.render_widget(block, main);
    let [header, body] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(inner);
    let disc = match &app.disc {
        Some((_, label)) => Line::from(vec![
            Span::styled(" Disc: ", Style::default().fg(MUTED)),
            Span::raw(label.as_str()),
        ]),
        None => Line::styled(" No disc chosen yet", Style::default().fg(MUTED)),
    };
    frame.render_widget(Paragraph::new(disc), header);
    match app.screen {
        Screen::Disc => disc_screen(frame, app, body),
        Screen::Options => options_screen(frame, app, body),
        Screen::Output => output_screen(frame, app, body),
        Screen::Patching => patching_screen(frame, app, body),
        Screen::Done => done_screen(frame, app, body),
        Screen::Failed => failed_screen(frame, app, body),
    }
    frame.render_widget(
        Paragraph::new(keys(app)).style(Style::default().fg(MUTED)),
        footer,
    );
}

fn keys(app: &App) -> &'static str {
    match (&app.prompt, app.screen) {
        (Some(Prompt::Overwrite), _) => " Y overwrite   N cancel",
        (Some(_), _) => " Enter confirm   Esc cancel   Ctrl+U clear",
        (None, Screen::Disc) if app.checking => " Please wait...",
        (None, Screen::Disc) => " Enter check disc   Ctrl+U clear   Esc quit",
        (None, Screen::Options) => {
            " Up/Down move  Space toggle  Enter continue  L load  S save  Esc back"
        }
        (None, Screen::Output) => " Enter patch   Ctrl+U clear   Esc back",
        (None, Screen::Patching) => " Please keep this window open",
        (None, Screen::Done) => " Enter exit",
        (None, Screen::Failed) => " Enter back to options",
    }
}

fn text(s: impl Into<String>) -> Paragraph<'static> {
    Paragraph::new(s.into()).wrap(Wrap { trim: true })
}

/// A one-line text field showing the end of the text when it is too long.
fn field(frame: &mut Frame, area: Rect, value: &str) {
    let width = area.width.saturating_sub(4) as usize;
    let chars: Vec<char> = value.chars().collect();
    let shown: String = if chars.len() > width {
        let tail: String = chars[chars.len() - width.saturating_sub(3)..].iter().collect();
        format!("...{tail}")
    } else {
        value.to_string()
    };
    let line = Line::from(vec![
        Span::styled("> ", Style::default().fg(ACCENT)),
        Span::styled(shown, Style::default().fg(Color::White).bg(Color::Blue)),
        Span::styled("_", Style::default().fg(ACCENT)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn notice(frame: &mut Frame, app: &App, area: Rect) {
    if let Some((message, error)) = &app.notice {
        let colour = if *error { ERROR } else { OK };
        frame.render_widget(
            text(message.clone()).style(Style::default().fg(colour)),
            area,
        );
    }
}

fn disc_screen(frame: &mut Frame, app: &App, area: Rect) {
    let [intro, input, _, status] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area.inner(ratatui::layout::Margin::new(1, 0)));
    frame.render_widget(
        text("Type or paste the path to your clean, original disc image (.iso, .gcm, .ciso or .rvz), then press Enter.\nTip: drag the file onto this window, or right-click and choose Paste."),
        intro,
    );
    field(frame, input, &app.disc_field);
    if app.checking {
        frame.render_widget(
            text("Checking your disc...").style(Style::default().fg(ACCENT)),
            status,
        );
    } else {
        notice(frame, app, status);
    }
}

fn options_screen(frame: &mut Frame, app: &App, area: Rect) {
    let area = area.inner(ratatui::layout::Margin::new(1, 0));
    let rows = app.rows();
    let [list, description, bottom] = Layout::vertical([
        Constraint::Length(rows.len() as u16),
        Constraint::Min(2),
        Constraint::Length(2),
    ])
    .areas(area);
    let current = app.current();
    let lines: Vec<Line> = rows
        .iter()
        .map(|row| match row {
            Row::Header(title) => Line::styled(*title, Style::default().fg(ACCENT)),
            Row::Item(opt) => {
                let indent = if opt.nested() { "    " } else { "  " };
                let label = if *opt == crate::app::Opt::EnemySeed {
                    format!("{indent}    {}: {}", opt.title(), app.seed_text)
                } else {
                    let mark = if app.enabled(*opt) { "[x]" } else { "[ ]" };
                    format!("{indent}{mark} {}", opt.title())
                };
                let style = if *opt == current {
                    Style::default().fg(Color::White).bg(Color::Blue)
                } else {
                    Style::default()
                };
                Line::styled(label, style)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), list);
    frame.render_widget(
        text(current.description()).style(Style::default().fg(MUTED)),
        description.inner(ratatui::layout::Margin::new(0, 0)),
    );
    match &app.prompt {
        Some(Prompt::Load(value)) => prompt_field(frame, bottom, "Load settings from:", value),
        Some(Prompt::Save(value)) => prompt_field(frame, bottom, "Save settings to:", value),
        _ => notice(frame, app, bottom),
    }
}

fn prompt_field(frame: &mut Frame, area: Rect, label: &str, value: &str) {
    let [top, line] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    frame.render_widget(Paragraph::new(label.to_string()), top);
    field(frame, line, value);
}

fn chosen(app: &App) -> Vec<String> {
    let mut chosen: Vec<String> = app
        .items()
        .into_iter()
        .filter(|opt| *opt != crate::app::Opt::EnemySeed && app.enabled(*opt))
        .map(|opt| format!("  - {}", opt.title()))
        .collect();
    if let Some(seed) = app.patch_selection().enemy_seed {
        chosen.push(format!("  - Enemy seed {seed}"));
    }
    chosen
}

fn output_screen(frame: &mut Frame, app: &App, area: Rect) {
    let area = area.inner(ratatui::layout::Margin::new(1, 0));
    let [label, input, _, summary, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new("Save the patched game as (edit if needed), then press Enter:"),
        label,
    );
    field(frame, input, &app.output_field);
    let mut lines = vec![Line::styled("Options:", Style::default().fg(ACCENT))];
    lines.extend(chosen(app).into_iter().map(Line::from));
    frame.render_widget(Paragraph::new(lines), summary);
    if let Some(Prompt::Overwrite) = app.prompt {
        frame.render_widget(
            text("That file already exists. Overwrite it? (Y/N)").style(Style::default().fg(ERROR)),
            bottom,
        );
    } else {
        notice(frame, app, bottom);
    }
}

fn patching_screen(frame: &mut Frame, app: &App, area: Rect) {
    let area = area.inner(ratatui::layout::Margin::new(1, 0));
    let [status, bar, _, info] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    let (label, fraction) = match &app.progress {
        Some(p) => (
            phase_label(p.phase),
            if p.total == 0 {
                0.0
            } else {
                (p.done as f64 / p.total as f64).clamp(0.0, 1.0)
            },
        ),
        None => ("Starting...".into(), 0.0),
    };
    frame.render_widget(Paragraph::new(label), status);
    let width = bar.width.saturating_sub(8) as usize;
    let filled = (fraction * width as f64).round() as usize;
    let line = Line::from(vec![
        Span::raw("["),
        Span::styled("#".repeat(filled), Style::default().fg(OK)),
        Span::styled(".".repeat(width - filled), Style::default().fg(MUTED)),
        Span::raw(format!("] {:>3}%", (fraction * 100.0) as u32)),
    ]);
    frame.render_widget(Paragraph::new(line), bar);
    let elapsed = app.started.elapsed().as_secs();
    frame.render_widget(
        text(format!(
            "Elapsed {:02}:{:02}\n\nRandom enemies can take several minutes. Closing this window stops the patch; your original disc is never changed.",
            elapsed / 60,
            elapsed % 60
        )),
        info,
    );
}

fn done_screen(frame: &mut Frame, app: &App, area: Rect) {
    let Some((path, outcome, seed)) = &app.outcome else {
        return;
    };
    let mut lines = vec![
        Line::styled("Done! Your patched game is ready.", Style::default().fg(OK)),
        Line::raw(""),
        Line::raw(format!("File:  {}", path.display())),
        Line::raw(format!("Size:  {:.2} GB", outcome.bytes as f64 / 1e9)),
        Line::raw(format!("MD5:   {}", outcome.md5)),
    ];
    if let Some(seed) = seed {
        lines.push(Line::raw(format!("Seed:  {seed}")));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        area.inner(ratatui::layout::Margin::new(1, 0)),
    );
}

fn failed_screen(frame: &mut Frame, app: &App, area: Rect) {
    let area = area.inner(ratatui::layout::Margin::new(1, 0));
    let [title, message] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);
    frame.render_widget(
        Paragraph::new("Patching failed.").style(Style::default().fg(ERROR)),
        title,
    );
    frame.render_widget(text(app.failure.clone()), message);
}
