//! Interactive viewer and editor: the board description in a vim-like
//! editor next to tabs that redraw the diagram on every change.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use edtui::{
    EditorEventHandler, EditorMode, EditorState, EditorTheme, EditorView, LineNumbers, Lines,
    SyntaxHighlighter,
};
use pinout::model::{Edge, Label, Pinout, ResolvedType};
use pinout::renderer::term::{label_chip, legend_entries, render_lines, StyledSpan, TermOptions};
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, HighlightSpacing, List, ListItem, ListState, Padding,
    Paragraph, Row, Table, TableState, Tabs, Wrap,
};
use ratatui::{DefaultTerminal, Frame};

const SYNTAX_THEME: &str = "base16-ocean-dark";

/// Screen colors, set explicitly so light terminal themes look the same.
const BG: Color = Color::Rgb(24, 24, 27);
const FG: Color = Color::Rgb(220, 220, 220);
const MUTED: Color = Color::Rgb(140, 142, 150);
const BAR: Color = Color::Rgb(38, 41, 48);
const ACCENT: Color = Color::Rgb(62, 92, 140);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Editor,
    Front,
    Back,
    Pins,
    Notes,
}

impl Tab {
    const ALL: [Tab; 5] = [Tab::Editor, Tab::Front, Tab::Back, Tab::Pins, Tab::Notes];

    fn title(self) -> &'static str {
        match self {
            Tab::Editor => "Editor",
            Tab::Front => "Front",
            Tab::Back => "Back",
            Tab::Pins => "Pins",
            Tab::Notes => "Notes",
        }
    }

    fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    fn step(self, by: isize) -> Tab {
        let count = Tab::ALL.len() as isize;
        Tab::ALL[(self.index() as isize + by).rem_euclid(count) as usize]
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Yaml,
    Json,
}

struct App {
    path: PathBuf,
    format: Format,
    tab: Tab,
    editor: EditorState,
    events: EditorEventHandler,
    /// Text as last parsed, to notice edits.
    text: String,
    /// Text as last saved, to notice unsaved changes.
    saved: String,
    /// The last description that parsed, shown while the text has errors.
    pinout: Option<Pinout>,
    error: Option<String>,
    /// Scroll offsets (row, column) of the front and back views.
    scroll: [(u16, u16); 2],
    /// Shorten labels to fit the window instead of scrolling.
    fit: bool,
    compact: bool,
    pins: TableState,
    /// First label column shown in the pins table.
    pin_column: usize,
    notes_scroll: u16,
    message: Option<String>,
    /// Set after a first quit request with unsaved changes.
    confirm_quit: bool,
    /// The vim-style command being typed after `:` in the editor.
    command: Option<String>,
    /// Label columns left out of the views and the pins table.
    hidden: BTreeSet<usize>,
    /// The column chooser, when open, with its selected row.
    columns_popup: Option<ListState>,
    quit: bool,
}

pub fn run(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut app = App::open(path)?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

impl App {
    fn open(path: &Path) -> Result<Self, Box<dyn Error>> {
        let format = match path.extension().and_then(|e| e.to_str()) {
            Some("json") => Format::Json,
            Some("yaml" | "yml") => Format::Yaml,
            _ => return Err("the editor opens .yaml, .yml and .json files".into()),
        };
        let text = std::fs::read_to_string(path)?;
        let mut app = App {
            path: path.to_path_buf(),
            format,
            tab: Tab::Front,
            editor: EditorState::new(Lines::from(text.as_str())),
            events: EditorEventHandler::default(),
            text: String::new(),
            saved: text.clone(),
            pinout: None,
            error: None,
            scroll: [(0, 0); 2],
            fit: false,
            compact: false,
            pins: TableState::default().with_selected(Some(0)),
            pin_column: 0,
            notes_scroll: 0,
            message: None,
            confirm_quit: false,
            command: None,
            hidden: BTreeSet::new(),
            columns_popup: None,
            quit: false,
        };
        app.parse(text);
        Ok(app)
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), Box<dyn Error>> {
        while !self.quit {
            terminal.draw(|frame| self.draw(frame))?;
            let event = event::read()?;
            self.handle(event)?;
        }
        Ok(())
    }

    fn buffer_text(&self) -> String {
        self.editor.lines.flatten(&Some('\n')).into_iter().collect()
    }

    /// Re-reads the description when the editor text changed.
    fn parse(&mut self, text: String) {
        if text == self.text {
            return;
        }
        let parsed = match self.format {
            Format::Yaml => Pinout::from_yaml_str(&text),
            Format::Json => Pinout::from_json_str(&text),
        };
        match parsed {
            Ok(pinout) => {
                self.pinout = Some(pinout);
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        self.text = text;
    }

    /// Trailing newlines do not count as a change; the editor buffer does
    /// not keep them the way the file does.
    fn modified(&self) -> bool {
        self.text.trim_end_matches('\n') != self.saved.trim_end_matches('\n')
    }

    fn save(&mut self) -> Result<(), Box<dyn Error>> {
        let mut text = self.buffer_text();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        std::fs::write(&self.path, &text)?;
        self.saved = text;
        self.message = Some(format!("{} written", self.path.display()));
        Ok(())
    }

    fn handle(&mut self, event: Event) -> Result<(), Box<dyn Error>> {
        let Event::Key(key) = event else {
            if self.tab == Tab::Editor {
                self.events.on_event(event, &mut self.editor);
            }
            return Ok(());
        };
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let editing = self.tab == Tab::Editor && self.editor.mode != EditorMode::Normal;
        self.message = None;
        // A quit request right after the warning confirms it, any other key
        // cancels it
        let confirmed = std::mem::take(&mut self.confirm_quit);

        if self.command.is_some() {
            return self.handle_command(key);
        }

        // Keys that work everywhere, even while typing
        match key.code {
            KeyCode::Char('s') if ctrl => return self.save(),
            KeyCode::Char('q') | KeyCode::Char('c') if ctrl => {
                self.request_quit(confirmed, "Ctrl-q");
                return Ok(());
            }
            KeyCode::F(n @ 1..=5) => {
                self.tab = Tab::ALL[n as usize - 1];
                self.columns_popup = None;
                return Ok(());
            }
            _ => {}
        }
        if self.columns_popup.is_some() {
            self.handle_columns(key);
            return Ok(());
        }
        if !editing {
            match key.code {
                KeyCode::Tab => {
                    self.tab = self.tab.step(1);
                    return Ok(());
                }
                KeyCode::BackTab => {
                    self.tab = self.tab.step(-1);
                    return Ok(());
                }
                _ => {}
            }
        }

        match self.tab {
            // Normal mode keys edtui leaves unbound
            Tab::Editor if key.code == KeyCode::Char(':') && !editing => {
                self.command = Some(String::new());
            }
            Tab::Editor if key.code == KeyCode::Char('q') && !editing => {
                self.request_quit(confirmed, "q");
            }
            Tab::Editor => {
                self.events.on_key_event(key, &mut self.editor);
                let text = self.buffer_text();
                self.parse(text);
            }
            Tab::Front | Tab::Back => self.handle_view(key, confirmed),
            Tab::Pins => match key.code {
                KeyCode::Down | KeyCode::Char('j') => self.pins.select_next(),
                KeyCode::Up | KeyCode::Char('k') => self.pins.select_previous(),
                KeyCode::Home | KeyCode::Char('g') => self.pins.select_first(),
                KeyCode::End | KeyCode::Char('G') => self.pins.select_last(),
                KeyCode::Right | KeyCode::Char('l') => self.pin_column += 1,
                KeyCode::Left | KeyCode::Char('h') => {
                    self.pin_column = self.pin_column.saturating_sub(1)
                }
                KeyCode::Char('q') => self.request_quit(confirmed, "q"),
                _ => {}
            },
            Tab::Notes => match key.code {
                KeyCode::Down | KeyCode::Char('j') => self.notes_scroll += 1,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.notes_scroll = self.notes_scroll.saturating_sub(1)
                }
                KeyCode::Char('q') => self.request_quit(confirmed, "q"),
                _ => {}
            },
        }
        Ok(())
    }

    /// Scrolling and toggles of the front and back views.
    fn handle_view(&mut self, key: KeyEvent, confirmed: bool) {
        let (row, column) = &mut self.scroll[(self.tab == Tab::Back) as usize];
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => *row += 1,
            KeyCode::Up | KeyCode::Char('k') => *row = row.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => *column += 4,
            KeyCode::Left | KeyCode::Char('h') => *column = column.saturating_sub(4),
            KeyCode::Home | KeyCode::Char('0') => (*row, *column) = (0, 0),
            KeyCode::Char('f') => self.fit = !self.fit,
            KeyCode::Char('c') => self.compact = !self.compact,
            KeyCode::Char('v') => {
                self.columns_popup = Some(ListState::default().with_selected(Some(0)))
            }
            KeyCode::Char('q') => self.request_quit(confirmed, "q"),
            _ => {}
        }
    }

    /// Keys while the column chooser is open.
    fn handle_columns(&mut self, key: KeyEvent) {
        let columns = self.pinout.as_ref().map(label_columns).unwrap_or_default();
        let Some(list) = self.columns_popup.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('v') | KeyCode::Char('q') => self.columns_popup = None,
            KeyCode::Down | KeyCode::Char('j') => list.select_next(),
            KeyCode::Up | KeyCode::Char('k') => list.select_previous(),
            KeyCode::Char(' ') | KeyCode::Enter => {
                let selected = list
                    .selected()
                    .unwrap_or(0)
                    .min(columns.len().saturating_sub(1));
                if let Some(column) = columns.get(selected) {
                    if !self.hidden.remove(&column.index) {
                        self.hidden.insert(column.index);
                    }
                }
            }
            KeyCode::Char('a') => self.hidden.clear(),
            _ => {}
        }
    }

    /// Keys while the `:` command line is open.
    fn handle_command(&mut self, key: KeyEvent) -> Result<(), Box<dyn Error>> {
        let Some(command) = self.command.as_mut() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => self.command = None,
            KeyCode::Backspace if command.is_empty() => self.command = None,
            KeyCode::Backspace => {
                command.pop();
            }
            KeyCode::Char(c) => command.push(c),
            KeyCode::Enter => {
                let command = self.command.take().unwrap_or_default();
                return self.run_command(command.trim());
            }
            _ => {}
        }
        Ok(())
    }

    /// The vim commands for writing and quitting.
    fn run_command(&mut self, command: &str) -> Result<(), Box<dyn Error>> {
        match command {
            "" => {}
            "w" => self.save()?,
            "q" if self.modified() => {
                self.message = Some("Unsaved changes: :w saves, :q! discards them".into())
            }
            "q" | "q!" | "qa" | "qa!" => self.quit = true,
            "wq" | "x" => {
                self.save()?;
                self.quit = true;
            }
            other => self.message = Some(format!("Not an editor command: {other}")),
        }
        Ok(())
    }

    /// Quits, or with unsaved changes warns first and quits when the same
    /// request follows right away.
    fn request_quit(&mut self, confirmed: bool, key: &str) {
        if self.modified() && !confirmed {
            self.confirm_quit = true;
            self.message = Some(format!(
                "Unsaved changes: Ctrl-s saves, {key} again discards them and quits"
            ));
        } else {
            self.quit = true;
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        // Paint the whole screen so the colors do not depend on the
        // terminal's own theme
        frame.render_widget(Block::new().style(Style::new().bg(BG).fg(FG)), frame.area());
        let [tabs, _, body, status] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let body = body.inner(Margin::new(1, 0));

        let titles = Tab::ALL
            .iter()
            .enumerate()
            .map(|(n, tab)| format!(" F{} {} ", n + 1, tab.title()));
        frame.render_widget(
            Tabs::new(titles)
                .select(self.tab.index())
                .style(Style::new().fg(MUTED).bg(BAR))
                .highlight_style(Style::new().fg(Color::White).bg(ACCENT).bold())
                .divider(Span::styled("│", Style::new().fg(MUTED))),
            tabs,
        );

        match self.tab {
            Tab::Editor => self.draw_editor(frame, body),
            Tab::Front | Tab::Back => self.draw_view(frame, body),
            Tab::Pins => self.draw_pins(frame, body),
            Tab::Notes => self.draw_notes(frame, body),
        }
        if matches!(self.tab, Tab::Front | Tab::Back) && self.columns_popup.is_some() {
            self.draw_columns(frame, body);
        }
        self.draw_status(frame, status);
    }

    fn draw_editor(&mut self, frame: &mut Frame, area: Rect) {
        let extension = match self.format {
            Format::Yaml => "yaml",
            Format::Json => "json",
        };
        let highlighter = SyntaxHighlighter::new(SYNTAX_THEME, extension).ok();
        let theme = EditorTheme::default()
            .base(
                Style::new()
                    .bg(Color::Rgb(43, 48, 59))
                    .fg(Color::Rgb(192, 197, 206)),
            )
            .line_numbers_style(
                Style::new()
                    .bg(Color::Rgb(43, 48, 59))
                    .fg(Color::Rgb(101, 115, 126)),
            );
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let theme = theme.block(panel(name));
        frame.render_widget(
            EditorView::new(&mut self.editor)
                .theme(theme)
                .syntax_highlighter(highlighter)
                .line_numbers(LineNumbers::Absolute),
            area,
        );
    }

    fn draw_view(&mut self, frame: &mut Frame, area: Rect) {
        let Some(pinout) = &self.pinout else {
            return self.draw_message(frame, area, "The description has no valid version yet");
        };
        let back = self.tab == Tab::Back;

        // The legend is a panel of its own so it stays in view while the
        // diagram scrolls
        let hidden: Vec<usize> = self.hidden.iter().copied().collect();
        let legend: Vec<Line> = legend_entries(
            pinout,
            &TermOptions {
                hidden_columns: hidden.clone(),
                ..TermOptions::default()
            },
        )
        .iter()
        .map(|(swatch, label)| Line::from(vec![to_span(swatch), Span::raw(format!(" {label}"))]))
        .collect();
        let legend_width = legend.iter().map(Line::width).max().unwrap_or(0) as u16 + 4;

        // Borders and padding of the diagram panel
        const CHROME: u16 = 4;
        const GAP: u16 = 1;
        let options = TermOptions {
            back,
            compact: self.compact,
            max_width: self
                .fit
                .then_some(area.width.saturating_sub(legend_width + GAP + CHROME) as usize),
            legend: false,
            frame: false,
            hidden_columns: hidden,
            ..TermOptions::default()
        };
        let lines: Vec<Line> = match render_lines(pinout, &options) {
            Ok(lines) => lines.iter().map(|l| to_line(l)).collect(),
            Err(error) => vec![Line::raw(error.to_string())],
        };
        // The diagram fills the screen next to the legend, both full height
        let [diagram_area, _, legend_area] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(GAP),
            Constraint::Length(legend_width),
        ])
        .areas(area);

        let title = if back {
            format!("{} (back)", pinout.title)
        } else {
            pinout.title.clone()
        };
        let (row, column) = self.scroll[back as usize];
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((row, column))
                .block(panel(title).padding(Padding::horizontal(1))),
            diagram_area,
        );

        if !legend.is_empty() {
            frame.render_widget(
                Paragraph::new(legend).block(panel("Legend").padding(Padding::horizontal(1))),
                legend_area,
            );
        }
    }

    /// The column chooser, drawn over the diagram.
    fn draw_columns(&mut self, frame: &mut Frame, area: Rect) {
        let Some(pinout) = &self.pinout else {
            return;
        };
        let columns = label_columns(pinout);
        let width = area.width.saturating_sub(4).min(80);
        let height = (columns.len() as u16 + 2).min(area.height.saturating_sub(2));
        let popup = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );

        // The selected row is painted here rather than with a List highlight
        // style, which would also repaint the color swatch
        let selected = self
            .columns_popup
            .as_ref()
            .and_then(ListState::selected)
            .map(|s| s.min(columns.len().saturating_sub(1)));
        let items: Vec<ListItem> = columns
            .iter()
            .enumerate()
            .map(|(row, column)| {
                let current = selected == Some(row);
                let paint = |style: Style| if current { style.bg(ACCENT) } else { style };
                let shown = !self.hidden.contains(&column.index);
                let mut spans = vec![
                    Span::styled(if current { "> " } else { "  " }, paint(Style::new())),
                    Span::styled(
                        if shown { "[x] " } else { "[ ] " },
                        paint(Style::new().fg(if shown { Color::Green } else { MUTED })),
                    ),
                    Span::styled(format!("{:>2}  ", column.index + 1), paint(Style::new())),
                ];
                if let Some(kind) = column.kinds.first() {
                    spans.push(to_span(&label_chip(
                        pinout,
                        &Label::new("  ", Some(&kind.name)),
                    )));
                    spans.push(Span::styled(" ", paint(Style::new())));
                }
                let kinds: Vec<&str> = column.kinds.iter().map(|k| k.label.as_str()).collect();
                spans.push(Span::styled(kinds.join(", "), paint(Style::new().bold())));
                spans.push(Span::styled(
                    format!(
                        "   {}{}",
                        column.samples.join(", "),
                        if column.more { ", …" } else { "" }
                    ),
                    paint(Style::new().fg(if current { FG } else { MUTED })),
                ));
                let mut line = Line::from(spans);
                if current {
                    let fill = (width as usize).saturating_sub(2 + line.width());
                    line.push_span(Span::styled(" ".repeat(fill), paint(Style::new())));
                }
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(
                panel("Columns")
                    .border_style(Style::new().fg(Color::White))
                    .title_bottom(
                        Line::styled(" Space toggle · a all · Esc close ", Style::new().fg(MUTED))
                            .right_aligned(),
                    ),
            )
            // Clear resets the cells to the terminal's colors, so both are set
            .style(Style::new().bg(BAR).fg(FG));
        frame.render_widget(Clear, popup);
        if let Some(state) = self.columns_popup.as_mut() {
            frame.render_stateful_widget(list, popup, state);
        }
    }

    fn draw_pins(&mut self, frame: &mut Frame, area: Rect) {
        let Some(pinout) = &self.pinout else {
            return self.draw_message(frame, area, "The description has no valid version yet");
        };
        let pins: Vec<(Edge, usize, Vec<Label>)> = Edge::ALL
            .iter()
            .flat_map(|edge| {
                pinout
                    .row(*edge)
                    .into_iter()
                    .enumerate()
                    .filter(|(_, pin)| !pin.is_empty())
                    .map(move |(index, pin)| (*edge, index, pin))
            })
            .collect();

        // The n-th label of every pin shares a column, as in the diagram;
        // columns that are blank on every pin are left out
        let count = pins.iter().map(|(_, _, pin)| pin.len()).max().unwrap_or(0);
        let widths: Vec<(usize, u16)> = (0..count)
            .map(|n| {
                let width = pins
                    .iter()
                    .filter_map(|(_, _, pin)| pin.get(n))
                    .map(|label| label.text.chars().count() as u16 + 2)
                    .filter(|w| *w > 2)
                    .max()
                    .unwrap_or(0);
                (n, width)
            })
            .filter(|(n, width)| *width > 0 && !self.hidden.contains(n))
            .collect();

        let [table_panel, detail_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(4)]).areas(area);
        let table_block = panel(format!("Pins ({})", pins.len()));
        let inner = table_block.inner(table_panel).inner(Margin::new(1, 0));

        // Show the label columns that fit, starting at the scroll offset
        const FIXED: u16 = 2 + 6 + 1 + 4 + 1;
        self.pin_column = self.pin_column.min(widths.len().saturating_sub(1));
        let mut room = inner.width.saturating_sub(FIXED);
        let shown: Vec<(usize, u16)> = widths
            .iter()
            .skip(self.pin_column)
            .take_while(|(_, width)| {
                let fits = *width <= room;
                room = room.saturating_sub(width + 1);
                fits
            })
            .copied()
            .collect();
        let hidden = widths.len() - shown.len();

        let rows: Vec<Row> = pins
            .iter()
            .map(|(edge, index, pin)| {
                let mut cells = vec![
                    Cell::from(edge.name()).style(Style::new().fg(MUTED)),
                    Cell::from(format!("{:>3}", index + 1)),
                ];
                for (n, width) in &shown {
                    let cell = match pin.get(*n).filter(|l| !l.text.is_empty()) {
                        Some(label) => {
                            let mut chip = label_chip(pinout, label);
                            chip.text = format!("{:<1$}", chip.text, *width as usize);
                            Cell::from(Line::from(to_span(&chip)))
                        }
                        None => Cell::from(""),
                    };
                    cells.push(cell);
                }
                Row::new(cells)
            })
            .collect();

        let mut constraints = vec![Constraint::Length(6), Constraint::Length(4)];
        constraints.extend(shown.iter().map(|(_, width)| Constraint::Length(*width)));

        let more = if hidden > 0 {
            format!(" {hidden} more label columns, h/l to scroll ")
        } else {
            String::new()
        };
        frame.render_widget(
            table_block.title_bottom(Line::styled(more, Style::new().fg(MUTED)).right_aligned()),
            table_panel,
        );
        let [header_area, _, table_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(inner);

        // The header is drawn apart from the table: "Labels" spans all label
        // columns, which a table header cannot do
        let labels = if self.pin_column > 0 {
            "… Labels"
        } else {
            "Labels"
        };
        frame.render_widget(
            Line::styled(
                format!("  {:<6} {:<4} {labels}", "Side", "Pos"),
                Style::new().fg(Color::White).bold(),
            ),
            header_area,
        );
        let table = Table::new(rows, constraints)
            .column_spacing(1)
            // Only the marker shows the selection, a row style would repaint the chips
            .highlight_symbol("> ")
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_stateful_widget(table, table_area, &mut self.pins);

        // The selected pin with the type of every label
        let selected = self.pins.selected().and_then(|i| pins.get(i));
        let mut spans = Vec::new();
        if let Some((_, _, pin)) = selected {
            for label in pin.iter().filter(|l| !l.text.is_empty()) {
                spans.push(to_span(&label_chip(pinout, label)));
                spans.push(Span::styled(
                    format!(" {}   ", pinout.resolve_type(label.kind.as_deref()).label),
                    Style::new().fg(MUTED),
                ));
            }
        }
        let title = match selected {
            Some((edge, index, _)) => format!("{} pin {}", edge.name(), index + 1),
            None => "No pin selected".to_string(),
        };
        frame.render_widget(
            Paragraph::new(Line::from(spans))
                .wrap(Wrap { trim: false })
                .block(panel(title).padding(Padding::horizontal(1))),
            detail_area,
        );
    }

    fn draw_notes(&mut self, frame: &mut Frame, area: Rect) {
        let Some(pinout) = &self.pinout else {
            return self.draw_message(frame, area, "The description has no valid version yet");
        };
        if pinout.notes.is_empty() {
            return self.draw_message(
                frame,
                area,
                "No notes; add a `notes:` list to the description",
            );
        }
        let mut lines = Vec::new();
        for note in &pinout.notes {
            if let Some(title) = &note.title {
                lines.push(Line::styled(
                    title.clone(),
                    Style::new().bold().fg(Color::Yellow),
                ));
            }
            lines.extend(note.lines.iter().map(|l| Line::raw(format!("  {l}"))));
            lines.push(Line::default());
        }
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((self.notes_scroll, 0))
                .block(panel("Notes").padding(Padding::uniform(1))),
            area,
        );
    }

    fn draw_message(&self, frame: &mut Frame, area: Rect, message: &str) {
        frame.render_widget(Paragraph::new(message).style(Style::new().fg(MUTED)), area);
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let name = self
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut spans = vec![Span::styled(
            format!(" {name}{} ", if self.modified() { " [+]" } else { "" }),
            Style::new().fg(Color::White).bg(ACCENT).bold(),
        )];
        match (&self.message, &self.error) {
            _ if self.command.is_some() => spans.push(Span::styled(
                format!(" :{}█", self.command.as_deref().unwrap_or_default()),
                Style::new().fg(Color::White),
            )),
            (Some(message), _) => spans.push(Span::styled(
                format!(" {message}"),
                Style::new().fg(Color::Yellow),
            )),
            (None, Some(error)) => spans.push(Span::styled(
                format!(" ✗ {}", error.lines().next().unwrap_or_default()),
                Style::new().fg(Color::Red),
            )),
            (None, None) => {
                spans.push(Span::styled(" ✓ valid", Style::new().fg(Color::Green)));
                let hint = match self.tab {
                    Tab::Editor => "  i insert · Esc normal · :w save · :q quit · Tab next tab",
                    Tab::Front | Tab::Back => {
                        "  hjkl scroll · v columns · f fit width · c compact · Tab next tab · q quit"
                    }
                    Tab::Pins => "  j/k select · h/l label columns · Tab next tab · q quit",
                    Tab::Notes => "  j/k scroll · Tab next tab · q quit",
                };
                spans.push(Span::styled(hint, Style::new().fg(MUTED)));
            }
        }
        frame.render_widget(Line::from(spans).style(Style::new().bg(BAR)), area);
    }
}

/// A label column of the board, described by what it holds.
struct LabelColumn {
    /// Position from the pin outwards, from 0.
    index: usize,
    /// Types found in the column, most common first.
    kinds: Vec<ResolvedType>,
    /// A few labels of the column.
    samples: Vec<String>,
    /// Whether the column has more labels than the samples.
    more: bool,
}

/// Every label column that holds at least one label.
fn label_columns(pinout: &Pinout) -> Vec<LabelColumn> {
    let mut columns: Vec<LabelColumn> = Vec::new();
    let mut counts: Vec<Vec<(String, usize)>> = Vec::new();
    for edge in Edge::ALL {
        for pin in pinout.row(edge) {
            for (index, label) in pin.iter().enumerate() {
                if label.text.is_empty() {
                    continue;
                }
                let position = match columns.iter().position(|c| c.index == index) {
                    Some(position) => position,
                    None => {
                        columns.push(LabelColumn {
                            index,
                            kinds: Vec::new(),
                            samples: Vec::new(),
                            more: false,
                        });
                        counts.push(Vec::new());
                        columns.len() - 1
                    }
                };
                let kind = pinout.resolve_type(label.kind.as_deref());
                match counts[position]
                    .iter_mut()
                    .find(|(name, _)| *name == kind.name)
                {
                    Some((_, count)) => *count += 1,
                    None => {
                        counts[position].push((kind.name.clone(), 1));
                        columns[position].kinds.push(kind);
                    }
                }
                if columns[position].samples.len() < 3 {
                    columns[position].samples.push(label.text.clone());
                } else {
                    columns[position].more = true;
                }
            }
        }
    }
    for (column, counts) in columns.iter_mut().zip(&counts) {
        let count = |name: &str| {
            counts
                .iter()
                .find(|(n, _)| n == name)
                .map_or(0, |(_, c)| *c)
        };
        column
            .kinds
            .sort_by_key(|kind| std::cmp::Reverse(count(&kind.name)));
    }
    columns.sort_by_key(|c| c.index);
    columns
}

/// A rounded panel with a bold title, used for every part of the screen.
fn panel<'a>(title: impl Into<String>) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(MUTED))
        .title(Span::styled(
            format!(" {} ", title.into()),
            Style::new().fg(Color::White).bold(),
        ))
}

fn to_span(span: &StyledSpan) -> Span<'static> {
    let mut style = Style::new();
    if let Some((r, g, b)) = span.fg {
        style = style.fg(Color::Rgb(r, g, b));
    }
    if let Some((r, g, b)) = span.bg {
        style = style.bg(Color::Rgb(r, g, b));
    }
    if span.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Span::styled(span.text.clone(), style)
}

fn to_line(spans: &[StyledSpan]) -> Line<'static> {
    Line::from(spans.iter().map(to_span).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn ctrl(c: char) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn typed(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle(key(KeyCode::Char(c))).unwrap();
        }
    }

    /// A copy of a board file the test may modify.
    fn scratch(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pinout-tui-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn screen(app: &mut App, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        dump(&buffer);
        buffer
    }

    fn text(buffer: &Buffer) -> String {
        let area = buffer.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Appends the screen as ANSI text to $PINOUT_TUI_DUMP for eyeballing.
    fn dump(buffer: &Buffer) {
        let Ok(path) = std::env::var("PINOUT_TUI_DUMP") else {
            return;
        };
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => Some((r, g, b)),
            Color::Black => Some((0, 0, 0)),
            Color::White => Some((255, 255, 255)),
            Color::Gray => Some((170, 170, 170)),
            Color::DarkGray => Some((100, 100, 100)),
            Color::Red => Some((220, 60, 60)),
            Color::Green => Some((80, 200, 80)),
            Color::Yellow => Some((230, 200, 60)),
            _ => None,
        };
        let mut out = String::new();
        let area = buffer.area;
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &buffer[(x, y)];
                let mut codes = vec!["0".to_string()];
                if cell.modifier.contains(Modifier::BOLD) {
                    codes.push("1".into());
                }
                if let Some((r, g, b)) = rgb(cell.fg) {
                    codes.push(format!("38;2;{r};{g};{b}"));
                }
                if let Some((r, g, b)) = rgb(cell.bg) {
                    codes.push(format!("48;2;{r};{g};{b}"));
                }
                out.push_str(&format!("\x1b[{}m{}", codes.join(";"), cell.symbol()));
            }
            out.push_str("\x1b[0m\n");
        }
        out.push('\n');
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap()
            .write_all(out.as_bytes())
            .unwrap();
    }

    const BOARD: &str = "title: Demo\npins:\n  left:\n    - [ \"GPIO1:gpio\", \"SDA:i2c\" ]\n  right:\n    - [ \"GND:gnd\" ]\nnotes:\n  - title: Features\n    lines: [ \"WiFi\" ]\n";

    #[test]
    fn editor_highlights_yaml() {
        assert!(SyntaxHighlighter::new(SYNTAX_THEME, "yaml").is_ok());
        assert!(SyntaxHighlighter::new(SYNTAX_THEME, "json").is_ok());
    }

    #[test]
    fn tabs_show_front_back_pins_and_notes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ATtiny85.yaml");
        let mut app = App::open(&path).unwrap();

        let front = text(&screen(&mut app, 170, 24));
        assert!(front.contains("F2 Front"), "{front}");
        assert!(front.contains("ATtiny85 (DIP-8)"), "{front}");
        assert!(front.contains("✓ valid"), "{front}");

        app.handle(key(KeyCode::Tab)).unwrap();
        let back = text(&screen(&mut app, 170, 24));
        assert!(back.contains("ATtiny85 (DIP-8) (back)"), "{back}");
        assert!(back.contains("Legend"), "{back}");

        // On a narrow screen the legend still shows whole
        let narrow = text(&screen(&mut app, 90, 24));
        assert!(narrow.contains("   Timer / PWM       │"), "{narrow}");
        assert!(narrow.contains("   Reset / debugWIRE │"), "{narrow}");

        app.handle(key(KeyCode::Tab)).unwrap();
        let pins = text(&screen(&mut app, 120, 24));
        assert!(pins.contains("Labels"), "{pins}");
        assert!(pins.contains("PCINT5"), "{pins}");
        assert!(pins.contains("left pin 1"), "{pins}");
        assert!(pins.contains("Reset / debugWIRE"), "{pins}");

        app.handle(key(KeyCode::Tab)).unwrap();
        let notes = text(&screen(&mut app, 120, 24));
        assert!(notes.contains("RSTDISBL"), "{notes}");

        app.handle(key(KeyCode::Tab)).unwrap();
        let editor = text(&screen(&mut app, 120, 24));
        assert!(editor.contains("title: \"ATtiny85 (DIP-8)\""), "{editor}");
    }

    #[test]
    fn pins_table_aligns_and_scrolls_label_columns() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ESP32-MAXIO.yaml");
        let mut app = App::open(&path).unwrap();
        app.handle(key(KeyCode::F(4))).unwrap();
        let wide = text(&screen(&mut app, 120, 20));
        assert!(wide.contains("more label columns"), "{wide}");

        // GPIO names share one column on every row
        let column = |screen: &str, name: &str| {
            screen
                .lines()
                .find(|l| l.contains(name))
                .and_then(|l| l.find(name))
                .unwrap()
        };
        assert_eq!(column(&wide, "GPIO36"), column(&wide, "GPIO39"));

        app.handle(key(KeyCode::Char('l'))).unwrap();
        let scrolled = text(&screen(&mut app, 120, 20));
        assert!(scrolled.contains("… Labels"), "{scrolled}");
    }

    #[test]
    fn quitting_with_changes_needs_a_second_request() {
        for (tab, quit) in [
            (KeyCode::F(2), key(KeyCode::Char('q'))),
            (KeyCode::F(1), ctrl('q')),
        ] {
            let path = scratch("quit.yaml", BOARD);
            let mut app = App::open(&path).unwrap();
            app.handle(key(KeyCode::F(1))).unwrap();
            typed(&mut app, "ddu");
            typed(&mut app, "x");
            assert!(app.modified());
            app.handle(key(tab)).unwrap();

            app.handle(quit.clone()).unwrap();
            assert!(!app.quit);
            assert!(app.message.as_deref().unwrap_or("").contains("again"));
            // Another key in between cancels the warning
            app.handle(key(KeyCode::Down)).unwrap();
            app.handle(quit.clone()).unwrap();
            assert!(!app.quit);
            app.handle(quit).unwrap();
            assert!(app.quit);
        }
    }

    /// Text in the terminal's default color is unreadable on the dark
    /// screen when the terminal uses a light theme.
    fn assert_colors_set(buffer: &Buffer, what: &str) {
        let area = buffer.area;
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &buffer[(x, y)];
                if !cell.symbol().trim().is_empty() {
                    assert!(
                        cell.fg != Color::Reset && cell.bg != Color::Reset,
                        "{what}: {:?} at ({x}, {y}) uses the terminal's colors\n{}",
                        cell.symbol(),
                        text(buffer)
                    );
                }
            }
        }
    }

    #[test]
    fn every_screen_sets_its_colors() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ATtiny85.yaml");
        let mut app = App::open(&path).unwrap();
        for tab in 1..=5 {
            app.handle(key(KeyCode::F(tab))).unwrap();
            assert_colors_set(
                &screen(&mut app, 170, 24),
                Tab::ALL[tab as usize - 1].title(),
            );
        }
        app.handle(key(KeyCode::F(2))).unwrap();
        app.handle(key(KeyCode::Char('v'))).unwrap();
        assert_colors_set(&screen(&mut app, 170, 24), "column chooser");
        app.handle(key(KeyCode::Esc)).unwrap();
        typed(&mut app, "q");
        app.handle(key(KeyCode::F(1))).unwrap();
        typed(&mut app, ":w");
        assert_colors_set(&screen(&mut app, 170, 24), "command line");
    }

    #[test]
    fn column_chooser_hides_label_columns() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("ATtiny85.yaml");
        let mut app = App::open(&path).unwrap();
        let before = text(&screen(&mut app, 170, 24));
        assert!(
            before.contains("MISO") && before.contains("SPI"),
            "{before}"
        );

        app.handle(key(KeyCode::Char('v'))).unwrap();
        let popup = text(&screen(&mut app, 170, 24));
        assert!(popup.contains("Columns"), "{popup}");
        assert!(
            popup.contains("[x]  4") && popup.contains("SPI   SCK, MISO, MOSI"),
            "{popup}"
        );
        assert!(popup.contains("I2C   SCL, SDA "), "{popup}");
        assert!(popup.contains("PB5, PB3, PB4, …"), "{popup}");

        // Column 4 holds SPI: pin number, port and analog come first
        typed(&mut app, "jjj ");
        assert!(app.hidden.contains(&3));
        let buffer = screen(&mut app, 170, 24);
        let popup = text(&buffer);
        assert!(popup.contains("> [ ]  4"), "{popup}");

        // The selected row keeps the color of its swatch
        let (x, y) = popup
            .lines()
            .enumerate()
            .find_map(|(y, line)| line.find("> [ ]  4").map(|x| (x, y)))
            .unwrap();
        let x = popup.lines().nth(y).unwrap()[..x].chars().count() + "> [ ]  4  ".len();
        assert_eq!(
            buffer[(x as u16, y as u16)].bg,
            Color::Rgb(0x77, 0x5e, 0xe8)
        );
        assert_eq!(buffer[(x as u16 - 2, y as u16)].bg, ACCENT);

        app.handle(key(KeyCode::Esc)).unwrap();
        assert!(app.columns_popup.is_none());
        let after = text(&screen(&mut app, 170, 24));
        assert!(!after.contains("MISO"), "{after}");
        assert!(!after.contains(" SPI "), "{after}");
        assert!(after.contains("USCK"), "{after}");

        // The pins table follows the same choice
        app.handle(key(KeyCode::F(4))).unwrap();
        let pins = text(&screen(&mut app, 120, 24));
        let table: String = pins.lines().take(14).collect::<Vec<_>>().join("\n");
        assert!(!table.contains("MISO"), "{pins}");

        app.handle(key(KeyCode::F(2))).unwrap();
        typed(&mut app, "va");
        assert!(app.hidden.is_empty());
    }

    #[test]
    fn editor_quits_with_q_and_vim_commands() {
        let run = |keys: &str| {
            let path = scratch("vim.yaml", BOARD);
            let mut app = App::open(&path).unwrap();
            app.handle(key(KeyCode::F(1))).unwrap();
            for part in keys.split('|') {
                match part {
                    "<Esc>" => app.handle(key(KeyCode::Esc)).unwrap(),
                    "<Enter>" => app.handle(key(KeyCode::Enter)).unwrap(),
                    text => typed(&mut app, text),
                }
            }
            let saved = std::fs::read_to_string(&path).unwrap();
            (app, saved)
        };

        let (app, _) = run("q");
        assert!(app.quit, "q in normal mode quits an unchanged file");

        let (app, _) = run(":q|<Enter>");
        assert!(app.quit);

        let (app, _) = run("x|:q|<Enter>");
        assert!(!app.quit, "changes need :q! or :w");
        assert!(app.message.as_deref().unwrap_or("").contains(":q!"));

        let (app, saved) = run("x|:q!|<Enter>");
        assert!(app.quit);
        assert_eq!(saved, BOARD);

        let (app, saved) = run("x|:wq|<Enter>");
        assert!(app.quit);
        assert_ne!(saved, BOARD);

        let (mut app, _) = run("x|:w|<Enter>");
        assert!(!app.quit && !app.modified());
        typed(&mut app, ":nope");
        let status = text(&screen(&mut app, 100, 12));
        assert!(status.contains(":nope"), "{status}");
        app.handle(key(KeyCode::Enter)).unwrap();
        assert!(app
            .message
            .as_deref()
            .unwrap_or("")
            .contains("Not an editor command"));

        // In insert mode q and : are text
        let (app, _) = run("i|q:|<Esc>");
        assert!(!app.quit && app.buffer_text().starts_with("q:title"));
    }

    #[test]
    fn edits_update_the_preview_and_save() {
        let path = scratch("board.yaml", BOARD);
        let mut app = App::open(&path).unwrap();
        app.handle(key(KeyCode::F(1))).unwrap();

        // Rename the board: go to "Demo" on the first line and change it
        typed(&mut app, "gg$");
        typed(&mut app, "ciw");
        typed(&mut app, "Pico");
        app.handle(key(KeyCode::Esc)).unwrap();
        assert_eq!(app.pinout.as_ref().unwrap().title, "Pico");
        assert!(app.modified());
        let editor = text(&screen(&mut app, 100, 16));
        assert!(editor.contains("board.yaml [+]"), "{editor}");

        // A broken line keeps the last valid board and reports the error
        typed(&mut app, "Go");
        typed(&mut app, "oops: [");
        app.handle(key(KeyCode::Esc)).unwrap();
        assert!(app.error.is_some());
        assert_eq!(app.pinout.as_ref().unwrap().title, "Pico");
        let editor = text(&screen(&mut app, 100, 16));
        assert!(editor.contains("✗"), "{editor}");
        typed(&mut app, "dd");
        assert!(app.error.is_none(), "{:?}", app.error);

        // Quitting with changes asks first, saving writes the file
        app.handle(ctrl('q')).unwrap();
        assert!(!app.quit);
        app.handle(ctrl('s')).unwrap();
        assert!(!app.modified());
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("title: Pico\n"));
        app.handle(key(KeyCode::F(2))).unwrap();
        app.handle(key(KeyCode::Char('q'))).unwrap();
        assert!(app.quit);
    }
}
