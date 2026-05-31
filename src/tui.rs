//! k9s-style live-scanning TUI for alchemy-cleaner.
//!
//! # Architecture
//! 1. `main` spawns one thread per checker (all run in parallel).
//! 2. Each thread sends `ScanMsg::Started(i)` then `ScanMsg::Done(i, result)`.
//! 3. The TUI event loop drains the channel on every tick, updating live.
//! 4. When all checkers finish, the health score is computed once and shown.
//! 5. `run()` returns `TuiOutput { pending, results }` when the user quits.
//!
//! # Layout
//! ```text
//! ╔─ 🍎 alchemy-cleaner ─────────────────── Score: 87/100 ──────────╗
//! ║  ⣾ Scanning  3/9  [████████░░░░░░░░░░░░░]  ✅ 2  ⚠  1  ○ 6     ║
//! ╠═ Checkers — 3/9 done ═══╦═ CPU ═════════════════════════════════╣
//! ║  1  ✅  system-info  —   ║  Load avg   0.42 / 0.71 / 0.80        ║
//! ║  2  ⚠   memory       -8  ║                                        ║
//! ║▶ 3  ⣾   cpu          ... ║  ⣾ Scanning...                        ║
//! ║  4  ○   disk          —  ║                                        ║
//! ╠──────────────────────────╩────────────────────────────────────────╣
//! ║  [↑↓] Navigate  [a] Actions  [PgUp/Dn] Scroll  [q] Quit          ║
//! ║  📄 Report: ~/Desktop/alchemy_health_report_20260531_120000.txt   ║
//! ╚═══════════════════════════════════════════════════════════════════╝
//! ```

use std::io;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};

use crate::actions::QUICK_WIN_COMMANDS;
use crate::checker::{CheckResult, Level};
use crate::scorer::{HealthScore, Verdict};

// ─── Public API ───────────────────────────────────────────────────────────────

/// Message sent from a background checker thread to the TUI event loop.
pub enum ScanMsg {
    /// The checker at index `i` has started executing.
    Started(usize),
    /// The checker at index `i` finished and produced `result`.
    Done(usize, CheckResult),
}

/// Returned by [`run`].
pub struct TuiOutput {
    /// Indices into [`QUICK_WIN_COMMANDS`] the user selected to run.
    pub pending: Vec<usize>,
    /// All check results in original checker order.
    pub results: Vec<CheckResult>,
}

// ─── Internal types ───────────────────────────────────────────────────────────

const SPINNERS: &[&str] = &["⣾", "⣽", "⣻", "⣷", "⣯", "⣟", "⡿", "⢿"];

/// Lifecycle of a single checker slot in the TUI.
enum ScanState {
    Pending,
    Running,
    Done(CheckResult),
}

enum Mode {
    Browse,
    Actions,
}

struct App {
    // ── Scan state ─────────────────────────────────────────────────────
    checker_names: Vec<String>,
    total:         usize,
    states:        Vec<ScanState>,
    done_count:    usize,
    scan_rx:       Receiver<ScanMsg>,
    /// Computed once when `done_count == total`; gates the Actions key.
    health:        Option<HealthScore>,

    // ── Animation ──────────────────────────────────────────────────────
    tick: u64,

    // ── UI ─────────────────────────────────────────────────────────────
    report_path:    Option<String>,
    table_state:    TableState,
    detail_scroll:  u16,
    mode:           Mode,
    action_checked: Vec<bool>,
    action_cursor:  usize,
    status_msg:     String,
    should_quit:    bool,
    pending:        Vec<usize>,
}

impl App {
    fn new(
        checker_names: Vec<String>,
        rx: Receiver<ScanMsg>,
        report_path: Option<String>,
    ) -> Self {
        let total = checker_names.len();
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            total,
            states:         (0..total).map(|_| ScanState::Pending).collect(),
            done_count:     0,
            scan_rx:        rx,
            health:         None,
            tick:           0,
            checker_names,
            report_path,
            table_state,
            detail_scroll:  0,
            mode:           Mode::Browse,
            action_checked: vec![false; QUICK_WIN_COMMANDS.len()],
            action_cursor:  0,
            status_msg:     String::new(),
            should_quit:    false,
            pending:        Vec::new(),
        }
    }

    fn selected(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    fn move_up(&mut self) {
        let i = self.selected();
        if i > 0 {
            self.table_state.select(Some(i - 1));
            self.detail_scroll = 0;
        }
    }

    fn move_down(&mut self) {
        let i = self.selected();
        if i + 1 < self.total {
            self.table_state.select(Some(i + 1));
            self.detail_scroll = 0;
        }
    }

    /// Drain all pending messages from the scan channel.
    /// Called every tick so multiple completions in one 80ms window are all applied.
    fn drain_channel(&mut self) {
        while let Ok(msg) = self.scan_rx.try_recv() {
            match msg {
                ScanMsg::Started(i) => {
                    self.states[i] = ScanState::Running;
                }
                ScanMsg::Done(i, result) => {
                    self.states[i] = ScanState::Done(result);
                    self.done_count += 1;
                    if self.done_count == self.total {
                        // Compute health once — never recomputed.
                        let ordered = self.ordered_results();
                        self.health = Some(HealthScore::compute(&ordered));
                    }
                }
            }
        }
    }

    /// Collect results in index order (all must be Done; panics otherwise).
    fn ordered_results(&self) -> Vec<CheckResult> {
        (0..self.total)
            .map(|i| match &self.states[i] {
                ScanState::Done(r) => r.clone(),
                _ => unreachable!("ordered_results called before all checkers done"),
            })
            .collect()
    }

    /// Collect results for TuiOutput; substitutes empty result for any incomplete slot.
    /// (Incomplete only if a checker thread panicked — TODO: handle thread panic.)
    fn collect_results(&self) -> Vec<CheckResult> {
        (0..self.total)
            .map(|i| match &self.states[i] {
                ScanState::Done(r) => r.clone(),
                _ => CheckResult::new(&self.checker_names[i]),
            })
            .collect()
    }

    /// Returns `(ok, warn, critical, pending_or_running)` counts.
    fn stats(&self) -> (usize, usize, usize, usize) {
        let (mut ok, mut warn, mut crit, mut pend) = (0usize, 0usize, 0usize, 0usize);
        for state in &self.states {
            match state {
                ScanState::Done(r) => match r.findings.iter().map(|f| &f.level).max().unwrap_or(&Level::Ok) {
                    Level::Ok       => ok   += 1,
                    Level::Warn     => warn += 1,
                    Level::Critical => crit += 1,
                },
                _ => pend += 1,
            }
        }
        (ok, warn, crit, pend)
    }
}

// ─── Panic-safe terminal restore ─────────────────────────────────────────────

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Launch the live-scanning TUI.
///
/// `checker_names` seeds the initial table (all Pending).
/// Background threads feed `rx` with [`ScanMsg`]s as work completes.
///
/// Returns [`TuiOutput`] with the user's selected action indices and all results.
pub fn run(
    checker_names: Vec<String>,
    rx: Receiver<ScanMsg>,
    report_path: Option<&str>,
) -> Result<TuiOutput> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard; // restores terminal on any exit path (including panic)

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(
        checker_names,
        rx,
        report_path.map(|s| s.to_string()),
    );

    loop {
        // 1. Drain all pending scan messages (multiple may arrive per 80ms tick)
        app.drain_channel();

        // 2. Advance animation counter (drives spinner, ~12.5 fps)
        app.tick = app.tick.wrapping_add(1);

        // 3. Render frame
        terminal.draw(|f| draw(f, &mut app))?;

        // 4. Poll keyboard (80ms timeout keeps animation smooth even without input)
        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.mode {
                        Mode::Browse  => on_browse_key(&mut app, key.code),
                        Mode::Actions => on_actions_key(&mut app, key.code),
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // _guard Drop fires here → disable_raw_mode + LeaveAlternateScreen
    let results = app.collect_results();
    let pending = std::mem::take(&mut app.pending);
    Ok(TuiOutput { pending, results })
}

// ─── Key handlers ─────────────────────────────────────────────────────────────

fn on_browse_key(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('q') | KeyCode::Esc  => app.should_quit = true,
        KeyCode::Up   | KeyCode::Char('k') => app.move_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_down(),
        KeyCode::Char('a') => {
            if app.health.is_some() {
                app.mode = Mode::Actions;
                app.action_cursor = 0;
                app.status_msg.clear();
            } else {
                app.status_msg =
                    "⏳  Scan in progress — actions available when all checkers complete.".into();
            }
        }
        KeyCode::PageUp   => app.detail_scroll = app.detail_scroll.saturating_sub(3),
        KeyCode::PageDown => app.detail_scroll = app.detail_scroll.saturating_add(3),
        _ => {}
    }
}

fn on_actions_key(app: &mut App, code: KeyCode) {
    let n = QUICK_WIN_COMMANDS.len();
    match code {
        KeyCode::Esc | KeyCode::Char('q') => app.mode = Mode::Browse,
        KeyCode::Up   | KeyCode::Char('k') => {
            if app.action_cursor > 0 { app.action_cursor -= 1; }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.action_cursor + 1 < n { app.action_cursor += 1; }
        }
        KeyCode::Char(' ') => {
            let i = app.action_cursor;
            app.action_checked[i] = !app.action_checked[i];
        }
        KeyCode::Enter => {
            let selected: Vec<usize> = app.action_checked
                .iter()
                .enumerate()
                .filter(|(_, &on)| on)
                .map(|(i, _)| i)
                .collect();

            if selected.is_empty() {
                app.status_msg = "Nothing selected — use [Space] to toggle an action.".into();
                app.mode = Mode::Browse;
            } else {
                app.pending = selected;
                app.should_quit = true;
            }
        }
        _ => {}
    }
}

// ─── Top-level render ─────────────────────────────────────────────────────────

fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // header: 5 content rows + 2 border rows
            Constraint::Min(8),    // body: checker table | detail panel
            Constraint::Length(4), // footer: 2 content rows + 2 border rows
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);
    draw_body(frame, app, chunks[1]);
    draw_footer(frame, app, chunks[2]);

    if matches!(app.mode, Mode::Actions) {
        draw_actions_modal(frame, app, area);
    }
}

// ─── Header ───────────────────────────────────────────────────────────────────

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let (ok, warn, crit, pend) = app.stats();
    let spinner = SPINNERS[(app.tick / 2) as usize % SPINNERS.len()];

    let (lines, border_color) = if let Some(h) = &app.health {
        // ── All done: show final score + verdict ──────────────────────
        let sc = verdict_color(&h.verdict);
        let bar = h.bar(36);
        let lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  🍎  alchemy-cleaner", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("  —  macOS System Health Diagnostic", Style::default().fg(Color::White).add_modifier(Modifier::DIM)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("  Score: {}/100  ", h.score), Style::default().fg(sc).add_modifier(Modifier::BOLD)),
                Span::styled(format!("[{bar}]"), Style::default().fg(sc)),
            ]),
            Line::from(vec![
                Span::styled(format!("  {} {}   ", h.verdict.icon(), h.verdict.label()), Style::default().fg(sc).add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ✅ {ok}  ⚠ {warn}  ✗ {crit}"), Style::default().fg(Color::DarkGray)),
            ]),
        ];
        (lines, sc)
    } else {
        // ── Scanning: show animated progress bar ──────────────────────
        let filled = if app.total > 0 { app.done_count * 36 / app.total } else { 0 };
        let empty  = 36usize.saturating_sub(filled);
        let bar    = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
        let lines  = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("  🍎  alchemy-cleaner", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("  —  macOS System Health Diagnostic", Style::default().fg(Color::White).add_modifier(Modifier::DIM)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("  {spinner} Scanning  {}/{} complete   ", app.done_count, app.total), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(format!("[{bar}]"), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled(format!("  ✅ {ok} passed  "), Style::default().fg(Color::Green)),
                Span::styled(format!("⚠ {warn} warn  "),   Style::default().fg(Color::Yellow)),
                Span::styled(format!("✗ {crit} critical  "), Style::default().fg(Color::Red)),
                Span::styled(format!("○ {pend} pending"), Style::default().fg(Color::DarkGray)),
            ]),
        ];
        (lines, Color::Cyan)
    };

    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        ),
        area,
    );
}

// ─── Body: checker table + detail panel ──────────────────────────────────────

fn draw_body(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_checker_table(frame, app, cols[0]);
    draw_detail_panel(frame, app, cols[1]);
}

fn draw_checker_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let spinner = SPINNERS[(app.tick / 2) as usize % SPINNERS.len()];

    // Materialise all cell content as owned Strings BEFORE the mutable borrow
    // of app.table_state required by render_stateful_widget.
    let rows: Vec<Row> = app
        .states
        .iter()
        .enumerate()
        .map(|(i, state)| {
            let num  = format!("{:>2}", i + 1);
            let name = app.checker_names[i].clone();

            match state {
                ScanState::Pending => Row::new(vec![
                    Cell::from(num).style(Style::default().fg(Color::DarkGray)),
                    Cell::from(" ○  ").style(Style::default().fg(Color::DarkGray)),
                    Cell::from(name).style(Style::default().fg(Color::DarkGray)),
                    Cell::from("  —").style(Style::default().fg(Color::DarkGray)),
                ]),

                ScanState::Running => Row::new(vec![
                    Cell::from(num).style(Style::default().fg(Color::DarkGray)),
                    Cell::from(format!(" {spinner}  ")).style(Style::default().fg(Color::Cyan)),
                    Cell::from(name).style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Cell::from("...").style(Style::default().fg(Color::DarkGray)),
                ]),

                ScanState::Done(r) => {
                    let max = r.findings.iter().map(|f| &f.level).max().unwrap_or(&Level::Ok);
                    let (icon, fg) = tui_level_style(max);
                    let pts: u32 = r.findings.iter().map(|f| f.score_deduction).sum();
                    let pts_str = if pts == 0 { "  —".to_string() } else { format!("-{pts}") };
                    let pts_fg = match max {
                        Level::Ok       => Color::DarkGray,
                        Level::Warn     => Color::Yellow,
                        Level::Critical => Color::Red,
                    };
                    Row::new(vec![
                        Cell::from(num).style(Style::default().fg(Color::DarkGray)),
                        Cell::from(format!(" {icon}  ")).style(Style::default().fg(fg)),
                        Cell::from(r.section.to_lowercase()).style(Style::default().fg(Color::White)),
                        Cell::from(pts_str).style(Style::default().fg(pts_fg)),
                    ])
                }
            }
        })
        .collect();

    let title = if app.health.is_some() {
        format!(" Checkers ({}) ", app.total)
    } else {
        format!(" Checkers  {}/{} ", app.done_count, app.total)
    };

    let header = Row::new(vec![
        Cell::from(" # ").style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        Cell::from(" St").style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        Cell::from("Checker").style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        Cell::from("Pts").style(Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
    ]);

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(5),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .title(Span::styled(title, Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
    )
    .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
    .highlight_symbol("▶ ");

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_detail_panel(frame: &mut Frame, app: &App, area: Rect) {
    let idx     = app.selected();
    let name    = &app.checker_names[idx];
    let title   = format!(" {} ", name.to_uppercase());
    let spinner = SPINNERS[(app.tick / 2) as usize % SPINNERS.len()];

    let lines: Vec<Line> = match &app.states[idx] {
        ScanState::Pending => vec![
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
                Span::styled("Waiting in queue...", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Checker will start when a thread slot becomes free.",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )),
        ],

        ScanState::Running => vec![
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("  {spinner}  "), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("Scanning...", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Running in parallel. Results appear here when complete.",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )),
        ],

        ScanState::Done(result) => {
            let mut out: Vec<Line> = Vec::new();

            // Key-value details block
            for (k, v) in &result.details {
                if v.is_empty() {
                    out.push(Line::from(Span::styled(
                        format!("  {k}"),
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                    )));
                } else {
                    out.push(Line::from(vec![
                        Span::styled(format!("  {k:<24}"), Style::default().fg(Color::DarkGray)),
                        Span::styled(v.clone(), Style::default().fg(Color::White)),
                    ]));
                }
            }
            if !result.details.is_empty() && !result.findings.is_empty() {
                out.push(Line::from(""));
            }

            // Findings with inline solutions
            for f in &result.findings {
                let (icon, fg) = tui_level_style(&f.level);
                out.push(Line::from(vec![
                    Span::styled(format!("  {icon}  "), Style::default().fg(fg)),
                    Span::styled(f.message.clone(), Style::default().fg(fg)),
                ]));
                if let Some(sol) = &f.solution {
                    for sol_line in sol.lines() {
                        out.push(Line::from(vec![
                            Span::raw("        "),
                            Span::styled(
                                sol_line.to_string(),
                                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                            ),
                        ]));
                    }
                    out.push(Line::from(""));
                }
            }
            out
        }
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue))
                    .title(Span::styled(title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
            )
            .scroll((app.detail_scroll, 0)),
        area,
    );
}

// ─── Footer ───────────────────────────────────────────────────────────────────

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let divider = Span::styled("  │  ", Style::default().fg(Color::DarkGray));

    let a_key = if app.health.is_some() {
        kh("[a]", "Actions")
    } else {
        Span::styled("[a] Actions (scanning…)", Style::default().fg(Color::DarkGray))
    };

    let keys = Line::from(vec![
        kh("[↑↓/jk]", "Navigate"),
        divider.clone(),
        a_key,
        divider.clone(),
        kh("[PgUp/Dn]", "Scroll"),
        divider,
        kh("[q/Esc]", "Quit"),
    ]);

    let status_text = if !app.status_msg.is_empty() {
        app.status_msg.clone()
    } else if let Some(path) = &app.report_path {
        format!("📄  Report: {path}")
    } else {
        "No report file — pass without --no-report to save.".into()
    };

    let status = Line::from(Span::styled(
        format!("  {status_text}"),
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(
        Paragraph::new(vec![keys, status]).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

// ─── Actions modal overlay ────────────────────────────────────────────────────

fn draw_actions_modal(frame: &mut Frame, app: &App, area: Rect) {
    let modal = centered_rect(62, 68, area);
    frame.render_widget(Clear, modal);

    let items: Vec<ListItem> = QUICK_WIN_COMMANDS
        .iter()
        .enumerate()
        .map(|(i, (label, cmd))| {
            let bullet   = if app.action_checked[i] { "◉" } else { "○" };
            let sudo_tag = if cmd.contains("sudo") { "  🔐" } else { "" };

            let (row_style, bullet_style) = if i == app.action_cursor {
                (
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::Yellow).bg(Color::Cyan),
                )
            } else if app.action_checked[i] {
                (Style::default().fg(Color::Green), Style::default().fg(Color::Green))
            } else {
                (Style::default().fg(Color::White), Style::default().fg(Color::DarkGray))
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("  {bullet} "), bullet_style),
                Span::styled(format!("{label}{sudo_tag}"), row_style),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(Span::styled(
                " ⚡ Quick Actions  —  [Space] toggle  [Enter] run  [Esc] back ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )),
    );

    frame.render_widget(list, modal);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Center a rectangle using percentage of the outer area.
fn centered_rect(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let margin_y = (100 - pct_y) / 2;
    let margin_x = (100 - pct_x) / 2;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(margin_y),
            Constraint::Percentage(pct_y),
            Constraint::Percentage(margin_y),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(margin_x),
            Constraint::Percentage(pct_x),
            Constraint::Percentage(margin_x),
        ])
        .split(vert[1])[1]
}

/// TUI-safe level icons (single-column-width symbols + color).
fn tui_level_style(level: &Level) -> (&'static str, Color) {
    match level {
        Level::Ok       => ("✓", Color::Green),
        Level::Warn     => ("⚠", Color::Yellow),
        Level::Critical => ("✗", Color::Red),
    }
}

fn verdict_color(v: &Verdict) -> Color {
    match v {
        Verdict::Healthy                        => Color::Green,
        Verdict::NeedsAttention                 => Color::Yellow,
        Verdict::Degraded | Verdict::Critical   => Color::Red,
    }
}

fn kh(key: &str, desc: &str) -> Span<'static> {
    Span::styled(
        format!("{key} {desc}"),
        Style::default().fg(Color::White),
    )
}
