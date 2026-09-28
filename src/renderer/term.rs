//! Terminal renderer: draws pinout diagrams as ANSI-colored text.
//!
//! The layout follows pinoutleaf: every pin function is a colored chip whose
//! background depends on what the function is (GPIO, analog, SPI, I2C, ...),
//! chips line up next to a board with one pad per pin, and a legend explains
//! the colors. Absolute coordinates, images and icons are ignored.

use crate::parser::types::{Command, Side};
use crate::renderer::svg::RenderError;

#[derive(Debug, Clone)]
pub struct TermOptions {
    /// Emit ANSI truecolor escape codes.
    pub color: bool,
    /// Only show these label columns (matched case-insensitively).
    pub labels: Option<Vec<String>>,
    /// Available width; boards wider than this are split up.
    pub max_width: Option<usize>,
    /// Board title. Defaults to the first message of the document.
    pub title: Option<String>,
    /// Put pins on consecutive lines instead of leaving a gap between them.
    pub compact: bool,
}

impl Default for TermOptions {
    fn default() -> Self {
        Self {
            color: true,
            labels: None,
            max_width: None,
            title: None,
            compact: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

const BOARD: Rgb = Rgb(28, 36, 46);
const BOARD_EDGE: Rgb = Rgb(120, 130, 140);
const PAD: Rgb = Rgb(255, 193, 7);
const FRAME: Rgb = Rgb(130, 130, 130);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    fg: Option<Rgb>,
    bg: Option<Rgb>,
    bold: bool,
}

impl Style {
    fn fg(color: Option<Rgb>) -> Self {
        Self {
            fg: color,
            ..Self::default()
        }
    }

    fn bold() -> Self {
        Self {
            bold: true,
            ..Self::default()
        }
    }

    fn board(fg: Rgb, bold: bool) -> Self {
        Self {
            fg: Some(fg),
            bg: Some(BOARD),
            bold,
        }
    }

    fn chip(background: Rgb) -> Self {
        Self {
            fg: Some(contrast(background)),
            bg: Some(background),
            bold: true,
        }
    }
}

/// Black or white, whichever reads better on the given background.
fn contrast(Rgb(r, g, b): Rgb) -> Rgb {
    let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    if luminance > 150.0 {
        Rgb(0, 0, 0)
    } else {
        Rgb(255, 255, 255)
    }
}

#[derive(Clone)]
struct Span {
    text: String,
    style: Style,
}

#[derive(Clone, Default)]
struct Line {
    spans: Vec<Span>,
    width: usize,
}

impl Line {
    fn push(&mut self, text: impl Into<String>, style: Style) {
        let text = text.into();
        self.width += text.chars().count();
        self.spans.push(Span { text, style });
    }

    fn pad(&mut self, n: usize) {
        if n > 0 {
            self.push(" ".repeat(n), Style::default());
        }
    }

    fn pad_to(&mut self, width: usize) {
        self.pad(width.saturating_sub(self.width));
    }

    fn append(&mut self, other: Line) {
        self.width += other.width;
        self.spans.extend(other.spans);
    }

    fn write(mut self, out: &mut String, color: bool) {
        while self
            .spans
            .last()
            .is_some_and(|s| s.style == Style::default() && s.text.trim().is_empty())
        {
            self.spans.pop();
        }
        for span in &self.spans {
            if !color || span.style == Style::default() {
                out.push_str(&span.text);
                continue;
            }
            let mut codes = Vec::new();
            if span.style.bold {
                codes.push("1".to_string());
            }
            if let Some(Rgb(r, g, b)) = span.style.fg {
                codes.push(format!("38;2;{r};{g};{b}"));
            }
            if let Some(Rgb(r, g, b)) = span.style.bg {
                codes.push(format!("48;2;{r};{g};{b}"));
            }
            out.push_str(&format!("\x1b[{}m{}\x1b[0m", codes.join(";"), span.text));
        }
        if !color {
            out.truncate(out.trim_end_matches(' ').len());
        }
        out.push('\n');
    }
}

fn width(lines: &[Line]) -> usize {
    lines.iter().map(|l| l.width).max().unwrap_or(0)
}

/// What a pin function is used for; decides the chip color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Pin,
    Gpio,
    Analog,
    Touch,
    Rtc,
    Power,
    Ground,
    Control,
    I2c,
    Spi,
    Uart,
    Serial,
    Pwm,
    Interrupt,
    SdCard,
    Ethernet,
    Clock,
    Debug,
    Logic,
    Other,
}

impl Category {
    fn name(self) -> &'static str {
        match self {
            Category::Pin => "Pin",
            Category::Gpio => "GPIO",
            Category::Analog => "Analog",
            Category::Touch => "Touch",
            Category::Rtc => "RTC",
            Category::Power => "Power",
            Category::Ground => "Ground",
            Category::Control => "Control",
            Category::I2c => "I2C",
            Category::Spi => "SPI",
            Category::Uart => "UART",
            Category::Serial => "Serial (SERCOM)",
            Category::Pwm => "PWM / Timer",
            Category::Interrupt => "Interrupt",
            Category::SdCard => "SD Card",
            Category::Ethernet => "Ethernet",
            Category::Clock => "Clock",
            Category::Debug => "Debug",
            Category::Logic => "Custom Logic",
            Category::Other => "Other",
        }
    }

    fn color(self) -> Rgb {
        match self {
            Category::Pin => Rgb(236, 239, 241),
            Category::Gpio => Rgb(124, 179, 66),
            Category::Analog => Rgb(239, 124, 0),
            Category::Touch => Rgb(236, 64, 122),
            Category::Rtc => Rgb(161, 136, 127),
            Category::Power => Rgb(211, 47, 47),
            Category::Ground => Rgb(66, 66, 66),
            Category::Control => Rgb(255, 179, 0),
            Category::I2c => Rgb(38, 166, 154),
            Category::Spi => Rgb(123, 97, 255),
            Category::Uart => Rgb(84, 110, 122),
            Category::Serial => Rgb(171, 71, 188),
            Category::Pwm => Rgb(253, 216, 53),
            Category::Interrupt => Rgb(0, 172, 193),
            Category::SdCard => Rgb(57, 73, 171),
            Category::Ethernet => Rgb(30, 136, 229),
            Category::Clock => Rgb(192, 202, 51),
            Category::Debug => Rgb(78, 52, 46),
            Category::Logic => Rgb(130, 119, 23),
            Category::Other => Rgb(158, 158, 158),
        }
    }

    /// Guess the category from the function name. The CSV format has no
    /// per-cell type, so this relies on common datasheet naming.
    fn classify(text: &str, column: usize) -> Category {
        let upper = text.to_ascii_uppercase();
        let token = upper.split_whitespace().next().unwrap_or("");
        // "ADC1:0" -> "ADC1", "HSPI:CLK" -> "HSPI"
        let head = token.split(':').next().unwrap_or("");
        // "GPIO21" -> "GPIO", "E14*" -> "E"
        let alpha = head.trim_end_matches(|c: char| c.is_ascii_digit() || c == '*');
        let numbered = alpha.len() < head.len();
        let is = |names: &[&str], value: &str| names.contains(&value);

        if is(&["GND", "VSS", "GROUND"], token) {
            Category::Ground
        } else if is(
            &["VCC", "VBUS", "VIN", "VDD", "VBAT", "BAT", "BAT+", "BAT-"],
            token,
        ) || (token.starts_with(|c: char| c.is_ascii_digit()) && token.contains('V'))
        {
            Category::Power
        } else if is(&["RST", "RESET", "NRST", "EN", "BOOT", "CHIP_PU"], token) {
            Category::Control
        } else if is(&["SDA", "SCL"], token) || alpha == "I2C" {
            Category::I2c
        } else if is(&["TX", "RX", "TXD", "RXD", "UART"], alpha)
            || (head.starts_with('U')
                && ["TXD", "RXD", "CTS", "RTS"]
                    .iter()
                    .any(|s| head.ends_with(s)))
        {
            Category::Uart
        } else if is(&["MISO", "MOSI", "SCK", "SCLK", "SS", "CS"], alpha)
            || ["HSPI", "VSPI", "FSPI", "SPI"]
                .iter()
                .any(|p| head.starts_with(p))
        {
            Category::Spi
        } else if head == "SD" || alpha == "SDIO" {
            Category::SdCard
        } else if alpha == "EMAC" {
            Category::Ethernet
        } else if is(&["SCOM", "SERCOM"], alpha) {
            Category::Serial
        } else if is(&["TC", "TCC", "PWM", "LEDC"], alpha) {
            Category::Pwm
        } else if is(&["EXTINT", "EINT", "INT"], alpha) {
            Category::Interrupt
        } else if alpha == "TOUCH" || (is(&["X", "Y"], alpha) && token.contains(':')) {
            Category::Touch
        } else if alpha == "RTC" {
            Category::Rtc
        } else if is(
            &[
                "ADC", "AIN", "DAC", "AC", "OA", "VREF", "VREFB", "SENSOR", "VDET",
            ],
            alpha,
        ) || (alpha == "A" && numbered)
        {
            Category::Analog
        } else if is(&["CLK", "XIN", "XOUT", "XTAL", "OSC", "32K"], alpha) {
            Category::Clock
        } else if is(
            &[
                "MTMS", "MTDI", "MTCK", "MTDO", "SWDIO", "SWCLK", "SWO", "TMS", "TDI", "TDO", "TCK",
            ],
            token,
        ) {
            Category::Debug
        } else if alpha == "CCL" {
            Category::Logic
        } else if (is(&["GPIO", "IO", "D"], alpha) && numbered)
            || (alpha.len() == 2 && alpha.starts_with('P') && numbered)
        {
            Category::Gpio
        } else if column == 0 {
            Category::Pin
        } else {
            Category::Other
        }
    }
}

struct PinSet {
    side: Side,
    packed: bool,
    anchor: (f32, f32),
    heading: Option<String>,
    rows: Vec<Row>,
}

struct Row {
    group: Option<String>,
    cells: Vec<String>,
    /// Free text of a PINTEXT, printed after the cells.
    message: Option<String>,
}

struct Note {
    heading: Option<String>,
    lines: Vec<(String, Option<Rgb>)>,
}

enum Block {
    Pins(PinSet),
    Note(Note),
}

/// A titled box from the draw phase, used as heading for the first pin set or
/// message placed inside it.
struct HeadingBox {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    title: String,
}

impl HeadingBox {
    fn contains(&self, x: f32, y: f32) -> bool {
        // Justification is not tracked, so accept the box extending either way
        (self.x - self.w..=self.x + self.w).contains(&x) && (self.y..=self.y + self.h).contains(&y)
    }
}

fn take_heading(boxes: &mut Vec<HeadingBox>, x: f32, y: f32) -> Option<String> {
    let index = boxes.iter().position(|b| b.contains(x, y))?;
    Some(boxes.remove(index).title)
}

struct Document {
    labels: Vec<String>,
    groups: Vec<(String, Option<Rgb>)>,
    blocks: Vec<Block>,
}

impl Document {
    fn group_color(&self, group: Option<&str>) -> Option<Rgb> {
        let group = group?;
        self.groups
            .iter()
            .find(|(name, _)| name == group)
            .and_then(|(_, color)| *color)
    }
}

/// Render the commands of a pinout description as terminal text.
pub fn render_terminal(commands: &[Command], options: &TermOptions) -> Result<String, RenderError> {
    let mut document = collect(commands)?;

    if let Some(filter) = &options.labels {
        for wanted in filter {
            if !document
                .labels
                .iter()
                .any(|l| l.eq_ignore_ascii_case(wanted))
            {
                return Err(RenderError::MissingData(format!(
                    "Unknown label '{}', available labels: {}",
                    wanted,
                    document.labels.join(", ")
                )));
            }
        }
    }

    // A leading message before any pins is the document title
    let mut title = options.title.clone();
    if let Some(Block::Note(note)) = document.blocks.first() {
        if title.is_none() {
            title = note
                .lines
                .iter()
                .map(|(t, _)| t.clone())
                .find(|t| !t.is_empty());
        }
        if note.lines.len() == 1 {
            document.blocks.remove(0);
        }
    }

    // Frame borders and padding take this many columns around a section
    const FRAME_WIDTH: usize = 6;
    let content_width = options.max_width.map(|w| w.saturating_sub(FRAME_WIDTH));

    let mut categories = Vec::new();
    let mut sections: Vec<(Option<String>, Vec<Line>)> = Vec::new();
    let mut notes: Vec<(usize, &Note)> = Vec::new();
    let mut board_title = title.clone();

    let mut blocks = document.blocks.iter().peekable();
    while let Some(block) = blocks.next() {
        let first = match block {
            Block::Note(note) => {
                notes.push((sections.len(), note));
                continue;
            }
            Block::Pins(set) => set,
        };
        let second = match blocks.peek() {
            Some(Block::Pins(second)) if is_pair(first, second) => {
                blocks.next();
                Some(second)
            }
            _ => None,
        };

        let heading = first.heading.clone().or_else(|| board_title.clone());
        let label = board_title.take().unwrap_or_default();
        let a = SideBlock::new(&document, first, options, &mut categories);
        let b = second.map(|s| SideBlock::new(&document, s, options, &mut categories));
        let (left, right) = match b {
            Some(b) if first.side == Side::Left => (Some(a), Some(b)),
            Some(b) => (Some(b), Some(a)),
            None if a.left => (Some(a), None),
            None => (None, Some(a)),
        };

        let lines = render_board(left.as_ref(), right.as_ref(), &label, options.compact);
        if left.is_some() && right.is_some() && content_width.is_some_and(|w| width(&lines) > w) {
            // Too wide side by side: give each half its own board
            let lines = render_board(left.as_ref(), None, &label, options.compact);
            sections.push((heading, lines));
            let lines = render_board(None, right.as_ref(), "", options.compact);
            sections.push((None, lines));
        } else {
            sections.push((heading, lines));
        }
    }

    categories.sort_by_key(|c| c.name());
    if let Some((_, lines)) = sections.first_mut() {
        let with_legend = beside(lines.clone(), legend_box(&categories), 4);
        *lines = if content_width.is_some_and(|w| width(&with_legend) > w) {
            let mut below = lines.clone();
            below.push(Line::default());
            below.extend(legend_inline(
                &categories,
                content_width.unwrap_or(usize::MAX),
            ));
            below
        } else {
            with_legend
        };
    }

    let mut out = String::new();
    if sections.is_empty() {
        if let Some(title) = &title {
            let mut line = Line::default();
            line.push(title.clone(), Style::bold());
            line.write(&mut out, options.color);
        }
    }
    let mut notes = notes.into_iter().peekable();
    for (index, (heading, lines)) in sections.into_iter().enumerate() {
        while let Some((_, note)) = notes.next_if(|(at, _)| *at <= index) {
            render_note(note, &mut out, options.color);
        }
        for line in frame(heading.as_deref(), lines) {
            line.write(&mut out, options.color);
        }
        out.push('\n');
    }
    for (_, note) in notes {
        render_note(note, &mut out, options.color);
    }

    Ok(out)
}

fn collect(commands: &[Command]) -> Result<Document, RenderError> {
    let mut document = Document {
        labels: Vec::new(),
        groups: Vec::new(),
        blocks: Vec::new(),
    };
    let mut boxes = Vec::new();
    let mut anchor = (0.0, 0.0);

    for command in commands {
        match command {
            Command::Labels { labels, .. } => {
                document.labels = labels.iter().map(|l| l.trim().to_string()).collect();
            }
            Command::Group {
                name,
                color,
                opacity,
            } => {
                let color = (*opacity > 0.0).then(|| parse_color(color)).flatten();
                document.groups.push((name.trim().to_string(), color));
            }
            Command::Box {
                x,
                y,
                box_width: Some(w),
                box_height: Some(h),
                message: Some(message),
                ..
            } if !clean(message).is_empty() => boxes.push(HeadingBox {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                title: clean(message),
            }),
            Command::Anchor { x, y } => anchor = (*x, *y),
            Command::PinSet { side, packed, .. } => document.blocks.push(Block::Pins(PinSet {
                side: *side,
                packed: *packed,
                anchor,
                heading: take_heading(&mut boxes, anchor.0, anchor.1),
                rows: Vec::new(),
            })),
            Command::Pin {
                group, attributes, ..
            } => current_pin_set(&mut document.blocks)?.rows.push(Row {
                group: group.as_deref().map(str::trim).map(String::from),
                cells: attributes.iter().map(|a| clean(a)).collect(),
                message: None,
            }),
            Command::PinText {
                pin_group,
                label,
                message,
                ..
            } => current_pin_set(&mut document.blocks)?.rows.push(Row {
                group: pin_group.as_deref().map(str::trim).map(String::from),
                cells: label.as_deref().map(clean).into_iter().collect(),
                message: Some(clean(message)).filter(|m| !m.is_empty()),
            }),
            Command::Message { x, y, .. } => document.blocks.push(Block::Note(Note {
                heading: take_heading(&mut boxes, x.unwrap_or(0.0), y.unwrap_or(0.0)),
                lines: Vec::new(),
            })),
            Command::Text { color, message, .. } => {
                if !matches!(document.blocks.last(), Some(Block::Note(_))) {
                    document.blocks.push(Block::Note(Note {
                        heading: None,
                        lines: Vec::new(),
                    }));
                }
                if let Some(Block::Note(note)) = document.blocks.last_mut() {
                    note.lines.push((clean(message), text_color(color)));
                }
            }
            _ => {}
        }
    }

    Ok(document)
}

fn current_pin_set(blocks: &mut [Block]) -> Result<&mut PinSet, RenderError> {
    blocks
        .iter_mut()
        .rev()
        .find_map(|b| match b {
            Block::Pins(set) => Some(set),
            Block::Note(_) => None,
        })
        .ok_or_else(|| RenderError::MissingData("PIN without prior PINSET".to_string()))
}

/// Two pin sets on opposite sides of the same row form one board.
fn is_pair(a: &PinSet, b: &PinSet) -> bool {
    let opposite = matches!(
        (a.side, b.side),
        (Side::Left, Side::Right) | (Side::Right, Side::Left)
    );
    opposite && b.heading.is_none() && (a.anchor.1 - b.anchor.1).abs() < 1.0
}

/// The chips of one pin set, laid out for one side of a board.
struct SideBlock {
    /// Left-hand blocks grow away from the board towards the left.
    left: bool,
    /// Chips of each row, aligned towards the board and `area` wide.
    rows: Vec<Line>,
    pads: Vec<Option<Rgb>>,
    area: usize,
}

impl SideBlock {
    fn new(
        document: &Document,
        set: &PinSet,
        options: &TermOptions,
        categories: &mut Vec<Category>,
    ) -> Self {
        let left = matches!(set.side, Side::Left | Side::Top);
        let shown = |i: usize| {
            document.labels.get(i).is_some_and(|label| {
                options
                    .labels
                    .as_ref()
                    .is_none_or(|filter| filter.iter().any(|f| f.eq_ignore_ascii_case(label)))
            })
        };

        // Unpacked sets keep a fixed width per column so equal functions line up
        let mut column_widths = vec![0; document.labels.len()];
        if !set.packed {
            for row in &set.rows {
                for (i, cell) in row.cells.iter().enumerate() {
                    if let Some(w) = column_widths.get_mut(i) {
                        *w = (*w).max(cell.chars().count() + 2);
                    }
                }
            }
        }

        let mut pieces_per_row = Vec::new();
        for row in &set.rows {
            let mut pieces = Vec::new();
            for (i, text) in row.cells.iter().enumerate() {
                if !shown(i) {
                    continue;
                }
                let mut piece = Line::default();
                if !text.is_empty() {
                    let category = Category::classify(text, i);
                    if !categories.contains(&category) {
                        categories.push(category);
                    }
                    piece.push(format!(" {text} "), Style::chip(category.color()));
                }
                if piece.width > 0 || !set.packed {
                    piece.pad_to(column_widths.get(i).copied().unwrap_or(0));
                    pieces.push(piece);
                }
            }
            if let Some(message) = &row.message {
                let mut piece = Line::default();
                piece.push(message.clone(), Style::default());
                pieces.push(piece);
            }
            if left {
                pieces.reverse();
            }
            let mut line = Line::default();
            for (n, piece) in pieces.into_iter().enumerate() {
                if n > 0 {
                    line.pad(1);
                }
                line.append(piece);
            }
            pieces_per_row.push(line);
        }

        let area = width(&pieces_per_row);
        let rows = pieces_per_row
            .into_iter()
            .map(|line| {
                let mut aligned = Line::default();
                if left {
                    aligned.pad(area - line.width);
                    aligned.append(line);
                } else {
                    aligned.append(line);
                    aligned.pad_to(area);
                }
                aligned
            })
            .collect();
        let pads = set
            .rows
            .iter()
            .map(|row| document.group_color(row.group.as_deref()))
            .collect();

        Self {
            left,
            rows,
            pads,
            area,
        }
    }
}

/// Draws a board with a pad for every pin and the chips of each side next to
/// it. Either side may be missing for single sided headers.
fn render_board(
    left: Option<&SideBlock>,
    right: Option<&SideBlock>,
    label: &str,
    compact: bool,
) -> Vec<Line> {
    let label_width = label.chars().count();
    let inner = if label.is_empty() {
        3
    } else {
        (label_width + 6).max(9)
    };
    let count = left
        .map_or(0, |b| b.rows.len())
        .max(right.map_or(0, |b| b.rows.len()));

    // Body lines: Some(pin index) or None for the gap between pins
    let mut body = Vec::new();
    for i in 0..count {
        if i > 0 && !compact {
            body.push(None);
        }
        body.push(Some(i));
    }
    let middle = body.len().saturating_sub(1) / 2;
    let left_width = left.map_or(0, |b| b.area + 1);
    let edge = Style::board(BOARD_EDGE, false);

    let mut lines = Vec::new();
    let mut top = Line::default();
    top.pad(left_width);
    top.push(
        format!("╭{}╮", "─".repeat(inner)),
        Style::fg(Some(BOARD_EDGE)),
    );
    lines.push(top);

    for (n, pin) in body.iter().enumerate() {
        let row = |side| pin_row(side, *pin);
        let mut line = Line::default();

        match row(left) {
            Some((block, i)) => {
                line.append(block.rows[i].clone());
                line.pad(1);
            }
            None => line.pad(left_width),
        }

        line.push("│", edge);
        let pad = |side, line: &mut Line| match row(side) {
            Some((block, i)) => line.push("●", Style::board(block.pads[i].unwrap_or(PAD), true)),
            None => line.push(" ", Style::board(BOARD, false)),
        };
        pad(left, &mut line);
        let text = if n == middle { label } else { "" };
        let space = inner - 2 - text.chars().count();
        line.push(" ".repeat(space / 2), Style::board(BOARD, false));
        line.push(text, Style::board(Rgb(235, 235, 235), true));
        line.push(" ".repeat(space - space / 2), Style::board(BOARD, false));
        pad(right, &mut line);
        line.push("│", edge);

        if let Some((block, i)) = row(right) {
            line.pad(1);
            line.append(block.rows[i].clone());
        }
        lines.push(line);
    }

    let mut bottom = Line::default();
    bottom.pad(left_width);
    bottom.push(
        format!("╰{}╯", "─".repeat(inner)),
        Style::fg(Some(BOARD_EDGE)),
    );
    lines.push(bottom);
    lines
}

fn pin_row(side: Option<&SideBlock>, pin: Option<usize>) -> Option<(&SideBlock, usize)> {
    side.zip(pin).filter(|(block, i)| *i < block.rows.len())
}

fn legend_entry(category: Category) -> Line {
    let mut line = Line::default();
    line.push("  ", Style::chip(category.color()));
    line.push(format!(" {}", category.name()), Style::default());
    line
}

fn legend_box(categories: &[Category]) -> Vec<Line> {
    let entries: Vec<Line> = categories.iter().map(|c| legend_entry(*c)).collect();
    let inner = width(&entries) + 2;
    let border = Style::fg(Some(FRAME));

    let mut lines = Vec::new();
    let mut top = Line::default();
    top.push(format!("╭{}╮", "─".repeat(inner)), border);
    lines.push(top);
    for entry in entries {
        let mut line = Line::default();
        line.push("│ ", border);
        line.append(entry);
        line.pad_to(inner + 1);
        line.push("│", border);
        lines.push(line);
    }
    let mut bottom = Line::default();
    bottom.push(format!("╰{}╯", "─".repeat(inner)), border);
    lines.push(bottom);
    lines
}

/// Legend entries flowed into lines no wider than `max`.
fn legend_inline(categories: &[Category], max: usize) -> Vec<Line> {
    let mut lines = vec![Line::default()];
    for category in categories {
        let entry = legend_entry(*category);
        let current = lines.last_mut().unwrap();
        if current.width > 0 && current.width + 3 + entry.width > max {
            lines.push(Line::default());
        }
        let current = lines.last_mut().unwrap();
        if current.width > 0 {
            current.pad(3);
        }
        current.append(entry);
    }
    lines
}

fn beside(left: Vec<Line>, right: Vec<Line>, gap: usize) -> Vec<Line> {
    let left_width = width(&left);
    let count = left.len().max(right.len());
    let mut left = left.into_iter();
    let mut right = right.into_iter();
    (0..count)
        .map(|_| {
            let mut line = left.next().unwrap_or_default();
            if let Some(other) = right.next() {
                line.pad_to(left_width + gap);
                line.append(other);
            }
            line
        })
        .collect()
}

/// Surrounds a section with a rounded border, the title set into its top edge.
fn frame(title: Option<&str>, content: Vec<Line>) -> Vec<Line> {
    let border = Style::fg(Some(FRAME));
    let title_width = title.map_or(0, |t| t.chars().count() + 3);
    let inner = (width(&content) + 4).max(title_width + 1);

    let mut lines = Vec::new();
    let mut top = Line::default();
    top.push("╭", border);
    if let Some(title) = title {
        top.push("─ ", border);
        top.push(title, Style::bold());
        top.push(" ", border);
    }
    top.push(format!("{}╮", "─".repeat(inner - title_width)), border);
    lines.push(top);

    let framed = |content: Line| {
        let mut line = Line::default();
        line.push("│", border);
        line.pad(2);
        line.append(content);
        line.pad_to(inner + 1);
        line.push("│", border);
        line
    };
    lines.push(framed(Line::default()));
    lines.extend(content.into_iter().map(framed));
    lines.push(framed(Line::default()));

    let mut bottom = Line::default();
    bottom.push(format!("╰{}╯", "─".repeat(inner)), border);
    lines.push(bottom);
    lines
}

fn render_note(note: &Note, out: &mut String, color: bool) {
    if let Some(heading) = &note.heading {
        let mut line = Line::default();
        line.push(heading.clone(), Style::bold());
        line.write(out, color);
    }
    for (text, fg) in &note.lines {
        let mut line = Line::default();
        line.push(text.clone(), Style::fg(*fg));
        line.write(out, color);
    }
    out.push('\n');
}

/// Collapse the escaped line breaks used in CSV cells into spaces and drop
/// quotes the CSV reader keeps when a quoted field follows a space.
fn clean(text: &str) -> String {
    let text = text.trim();
    let text = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text);
    text.replace("\\\\n", " ")
        .replace("\\n", " ")
        .replace('\n', " ")
        .trim()
        .to_string()
}

/// Text colors meant for a white page; black and white would vanish on one of
/// the terminal themes, so those fall back to the terminal foreground.
fn text_color(name: &str) -> Option<Rgb> {
    match name.trim().to_ascii_lowercase().as_str() {
        "black" | "white" | "none" | "" => None,
        other => parse_color(other),
    }
}

fn parse_color(name: &str) -> Option<Rgb> {
    let name = name.trim().to_ascii_lowercase();
    if let Some(hex) = name.strip_prefix('#') {
        let expanded: String = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 => hex.to_string(),
            _ => return None,
        };
        let value = u32::from_str_radix(&expanded, 16).ok()?;
        return Some(rgb(value));
    }
    CSS_COLORS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, value)| rgb(*value))
}

fn rgb(value: u32) -> Rgb {
    Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

#[rustfmt::skip]
const CSS_COLORS: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff), ("antiquewhite", 0xfaebd7), ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4), ("azure", 0xf0ffff), ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4), ("black", 0x000000), ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff), ("blueviolet", 0x8a2be2), ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887), ("cadetblue", 0x5f9ea0), ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e), ("coral", 0xff7f50), ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc), ("crimson", 0xdc143c), ("cyan", 0x00ffff),
    ("darkblue", 0x00008b), ("darkcyan", 0x008b8b), ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9), ("darkgreen", 0x006400), ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b), ("darkmagenta", 0x8b008b), ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00), ("darkorchid", 0x9932cc), ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a), ("darkseagreen", 0x8fbc8f), ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f), ("darkslategrey", 0x2f4f4f), ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3), ("deeppink", 0xff1493), ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969), ("dimgrey", 0x696969), ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222), ("floralwhite", 0xfffaf0), ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff), ("gainsboro", 0xdcdcdc), ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700), ("goldenrod", 0xdaa520), ("gray", 0x808080),
    ("green", 0x008000), ("greenyellow", 0xadff2f), ("grey", 0x808080),
    ("honeydew", 0xf0fff0), ("hotpink", 0xff69b4), ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082), ("ivory", 0xfffff0), ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa), ("lavenderblush", 0xfff0f5), ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd), ("lightblue", 0xadd8e6), ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff), ("lightgoldenrodyellow", 0xfafad2), ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90), ("lightgrey", 0xd3d3d3), ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a), ("lightseagreen", 0x20b2aa), ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899), ("lightslategrey", 0x778899), ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0), ("lime", 0x00ff00), ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6), ("magenta", 0xff00ff), ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa), ("mediumblue", 0x0000cd), ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db), ("mediumseagreen", 0x3cb371), ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a), ("mediumturquoise", 0x48d1cc), ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970), ("mintcream", 0xf5fffa), ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5), ("navajowhite", 0xffdead), ("navy", 0x000080),
    ("oldlace", 0xfdf5e6), ("olive", 0x808000), ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500), ("orangered", 0xff4500), ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa), ("palegreen", 0x98fb98), ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093), ("papayawhip", 0xffefd5), ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f), ("pink", 0xffc0cb), ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6), ("purple", 0x800080), ("rebeccapurple", 0x663399),
    ("red", 0xff0000), ("rosybrown", 0xbc8f8f), ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513), ("salmon", 0xfa8072), ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57), ("seashell", 0xfff5ee), ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0), ("skyblue", 0x87ceeb), ("slateblue", 0x6a5acd),
    ("slategray", 0x708090), ("slategrey", 0x708090), ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f), ("steelblue", 0x4682b4), ("tan", 0xd2b48c),
    ("teal", 0x008080), ("thistle", 0xd8bfd8), ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0), ("violet", 0xee82ee), ("wheat", 0xf5deb3),
    ("white", 0xffffff), ("whitesmoke", 0xf5f5f5), ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::{JustifyX, JustifyY, PinType, WireType};

    fn pin(attributes: &[&str]) -> Command {
        Command::Pin {
            wire: Some(WireType::Digital),
            pin_type: Some(PinType::IO),
            group: None,
            attributes: attributes.iter().map(|a| a.to_string()).collect(),
        }
    }

    fn pin_set(side: Side) -> Command {
        Command::PinSet {
            side,
            packed: true,
            justify_x: JustifyX::Center,
            justify_y: JustifyY::Center,
            line_step: 25.0,
            pin_width: 60.0,
            group_width: 80.0,
            leader_offset: 10.0,
            column_gap: 5.0,
            leader_h_step: 2.0,
        }
    }

    fn commands() -> Vec<Command> {
        vec![
            Command::Labels {
                default: "DEFAULT".to_string(),
                pin_type: None,
                group: None,
                labels: vec!["GPIO".to_string(), "Function".to_string()],
            },
            Command::Draw,
            Command::Anchor { x: 50.0, y: 100.0 },
            pin_set(Side::Left),
            pin(&["GPIO5", "MISO"]),
            pin(&["GPIO6", ""]),
            Command::Anchor { x: 150.0, y: 100.0 },
            pin_set(Side::Right),
            pin(&["5V", ""]),
            pin(&["GND", ""]),
        ]
    }

    fn plain(options: TermOptions) -> String {
        let options = TermOptions {
            color: false,
            ..options
        };
        render_terminal(&commands(), &options).unwrap()
    }

    #[test]
    fn renders_board_with_legend() {
        let out = plain(TermOptions {
            title: Some("MCU".to_string()),
            ..TermOptions::default()
        });
        let expected = "\
╭─ MCU ───────────────────────────────────────────────╮
│                                                     │
│                 ╭─────────╮          ╭───────────╮  │
│   MISO   GPIO5  │●       ●│  5V      │    GPIO   │  │
│                 │   MCU   │          │    Ground │  │
│          GPIO6  │●       ●│  GND     │    Power  │  │
│                 ╰─────────╯          │    SPI    │  │
│                                      ╰───────────╯  │
│                                                     │
╰─────────────────────────────────────────────────────╯

";
        assert_eq!(out, expected, "\n{out}");
    }

    #[test]
    fn compact_board_has_no_gaps_between_pins() {
        let out = plain(TermOptions {
            compact: true,
            ..TermOptions::default()
        });
        assert!(out.contains("GPIO6  │● ●│  GND"), "\n{out}");
    }

    #[test]
    fn label_filter_hides_columns_and_rejects_unknown_labels() {
        let out = plain(TermOptions {
            labels: Some(vec!["gpio".to_string()]),
            ..TermOptions::default()
        });
        assert!(!out.contains("MISO"));

        let options = TermOptions {
            labels: Some(vec!["Nope".to_string()]),
            ..TermOptions::default()
        };
        assert!(render_terminal(&commands(), &options).is_err());
    }

    #[test]
    fn classifies_function_names() {
        let cases = [
            ("GPIO21 EXTINT:6", 2, Category::Gpio),
            ("PA05 AIN:1", 0, Category::Gpio),
            ("ADC1:0", 3, Category::Analog),
            ("A5", 1, Category::Analog),
            ("TOUCH9", 4, Category::Touch),
            ("X:1/Y:7", 4, Category::Touch),
            ("HSPI:CLK", 6, Category::Spi),
            ("MISO", 1, Category::Spi),
            ("VSPICLK", 6, Category::Spi),
            ("SDA", 0, Category::I2c),
            ("U0TXD PA01", 1, Category::Uart),
            ("TX", 0, Category::Uart),
            ("SCOM3:0 SCOM5:0", 6, Category::Serial),
            ("TC0:WO0 TCC0:WO4", 7, Category::Pwm),
            ("EXTINT:2", 2, Category::Interrupt),
            ("SD:CLK", 7, Category::SdCard),
            ("EMAC TXD2", 8, Category::Ethernet),
            ("32K XP", 1, Category::Clock),
            ("MTMS", 1, Category::Debug),
            ("CCL0:IN0", 8, Category::Logic),
            ("RTC:00", 5, Category::Rtc),
            ("3.3VP", 0, Category::Power),
            ("GND", 0, Category::Ground),
            ("RST", 0, Category::Control),
            ("E14*", 0, Category::Pin),
            ("WHATEVER", 3, Category::Other),
        ];
        for (text, column, expected) in cases {
            assert_eq!(Category::classify(text, column), expected, "{text}");
        }
    }

    #[test]
    fn parses_named_and_hex_colors() {
        assert_eq!(parse_color("DeepSkyBlue"), Some(Rgb(0, 191, 255)));
        assert_eq!(parse_color("#f80"), Some(Rgb(255, 136, 0)));
        assert_eq!(parse_color("none"), None);
    }
}
