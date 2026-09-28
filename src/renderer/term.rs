//! Terminal renderer for the [`Pinout`] model: draws the board as
//! ANSI-colored text.
//!
//! The layout follows pinoutleaf: every label is a chip in the colors of its
//! type, chips line up next to a board with one pad per pin, and a legend
//! explains the colors. Top and bottom rows stack their chips vertically.

use crate::model::{is_builtin_type, Edge, Label, Pinout, PinoutError, ResolvedType};

#[derive(Debug, Clone)]
pub struct TermOptions {
    /// Emit ANSI truecolor escape codes.
    pub color: bool,
    /// Available width; labels are shortened until the diagram fits.
    pub max_width: Option<usize>,
    /// Put pins on consecutive lines instead of leaving a gap between them.
    pub compact: bool,
    /// Draw the back of the board instead of the front.
    pub back: bool,
    /// Only show labels of these types.
    pub types: Option<Vec<String>>,
    /// Print the board's notes below the diagram.
    pub notes: bool,
    /// Flow the labels of each pin next to each other instead of aligning
    /// them in columns.
    pub packed: bool,
    /// Shorten labels longer than this. Labels are also shortened as far as
    /// needed to fit `max_width`, down to a few characters.
    pub max_label: Option<usize>,
}

impl Default for TermOptions {
    fn default() -> Self {
        Self {
            color: true,
            max_width: None,
            compact: false,
            back: false,
            types: None,
            notes: false,
            packed: false,
            max_label: None,
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
    fn fg(color: Rgb) -> Self {
        Self {
            fg: Some(color),
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

    /// Colors of a type; unreadable color values fall back to black with
    /// contrasting text.
    fn chip(kind: &ResolvedType) -> Self {
        let bg = parse_color(&kind.bgcolor).unwrap_or(Rgb(0, 0, 0));
        Self {
            fg: Some(parse_color(&kind.fgcolor).unwrap_or_else(|| contrast(bg))),
            bg: Some(bg),
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

/// Render the front (or back) of a board as terminal text.
pub fn render_terminal(pinout: &Pinout, options: &TermOptions) -> Result<String, PinoutError> {
    if let Some(filter) = &options.types {
        for wanted in filter {
            if !is_builtin_type(wanted) && !pinout.types.contains_key(wanted) {
                let used: Vec<String> = pinout.used_types().into_iter().map(|t| t.name).collect();
                return Err(PinoutError::Invalid(format!(
                    "unknown type '{wanted}', the board uses: {}",
                    used.join(", ")
                )));
            }
        }
    }

    let pinout = if options.back {
        pinout.flipped()
    } else {
        pinout.clone()
    };
    let mut cells = Cells {
        pinout: &pinout,
        types: options.types.as_deref(),
        max_label: options.max_label,
    };

    // Frame borders and padding take this many columns around the content
    const FRAME_WIDTH: usize = 6;
    let content_width = options.max_width.map(|w| w.saturating_sub(FRAME_WIDTH));
    let used = cells.used_types();
    let mut content = layout(&cells, &used, options);

    // Shorten long labels step by step until the diagram fits
    if let Some(max) = content_width {
        let longest = cells.longest_label();
        let mut limit = cells.max_label.unwrap_or(longest).min(longest);
        while width(&content) > max && limit > MIN_LABEL {
            limit -= 1;
            cells.max_label = Some(limit);
            content = layout(&cells, &used, options);
        }
    }

    let title = if options.back {
        format!("{} (back)", pinout.title)
    } else {
        pinout.title.clone()
    };
    let mut out = String::new();
    let title = Some(title.as_str()).filter(|t| !t.is_empty());
    for line in frame(title, content) {
        line.write(&mut out, options.color);
    }
    if options.notes {
        for note in &pinout.notes {
            out.push('\n');
            if let Some(title) = &note.title {
                let mut line = Line::default();
                line.push(title.clone(), Style::bold());
                line.write(&mut out, options.color);
            }
            for text in &note.lines {
                let mut line = Line::default();
                line.push(text.clone(), Style::default());
                line.write(&mut out, options.color);
            }
        }
    }
    Ok(out)
}

/// Labels are never shortened below this many characters.
const MIN_LABEL: usize = 4;

/// The board with the legend beside it.
fn layout(cells: &Cells, used: &[ResolvedType], options: &TermOptions) -> Vec<Line> {
    let left = Side::new(cells, Edge::Left, options.packed);
    let right = Side::new(cells, Edge::Right, options.packed);
    let top = Stack::new(cells, Edge::Top);
    let bottom = Stack::new(cells, Edge::Bottom);
    let board = render_board(
        &left,
        &right,
        &top,
        &bottom,
        &cells.pinout.title,
        options.compact,
    );

    beside(board, legend_box(used), 4)
}

/// Turns labels into colored chips, applying the type filter and the label
/// length limit.
struct Cells<'a> {
    pinout: &'a Pinout,
    types: Option<&'a [String]>,
    max_label: Option<usize>,
}

impl Cells<'_> {
    /// The type of a label that is shown, or `None` for hidden and blank ones.
    fn visible(&self, label: &Label) -> Option<ResolvedType> {
        if label.text.is_empty() {
            return None;
        }
        let kind = self.pinout.resolve_type(label.kind.as_deref());
        self.types
            .is_none_or(|filter| filter.contains(&kind.name))
            .then_some(kind)
    }

    fn chip(&self, label: &Label) -> Option<Chip> {
        let kind = self.visible(label)?;
        Some(Chip {
            text: shorten(&label.text, self.max_label),
            style: Style::chip(&kind),
        })
    }

    /// Chips of a pin in order, `None` where a label is hidden or blank.
    fn pin(&self, pin: &[Label]) -> Vec<Option<Chip>> {
        pin.iter().map(|label| self.chip(label)).collect()
    }

    fn used_types(&self) -> Vec<ResolvedType> {
        let mut used: Vec<ResolvedType> = Vec::new();
        for edge in Edge::ALL {
            for label in self.pinout.row(edge).iter().flatten() {
                if let Some(kind) = self.visible(label) {
                    if !used.iter().any(|u| u.name == kind.name) {
                        used.push(kind);
                    }
                }
            }
        }
        used.sort_by(|a, b| a.label.cmp(&b.label));
        used
    }

    fn longest_label(&self) -> usize {
        Edge::ALL
            .iter()
            .flat_map(|edge| self.pinout.row(*edge))
            .flatten()
            .filter(|label| self.visible(label).is_some())
            .map(|label| label.text.chars().count())
            .max()
            .unwrap_or(0)
    }
}

/// A label drawn as a colored box.
#[derive(Clone)]
struct Chip {
    text: String,
    style: Style,
}

impl Chip {
    fn width(&self) -> usize {
        self.text.chars().count() + 2
    }

    /// The chip with its background stretched to `width` columns.
    fn line(&self, width: usize) -> Line {
        let mut line = Line::default();
        let fill = width.saturating_sub(self.width());
        line.push(format!(" {}{} ", self.text, " ".repeat(fill)), self.style);
        line
    }
}

/// Cuts text longer than `limit` characters out of its middle, keeping the
/// prefix and the pin number at the end: `GPIO36` becomes `GP…36`.
fn shorten(text: &str, limit: Option<usize>) -> String {
    let chars: Vec<char> = text.chars().collect();
    match limit {
        Some(limit) if chars.len() > limit => {
            let kept = limit.saturating_sub(1);
            let tail = kept / 2;
            let head = kept - tail;
            let head: String = chars[..head].iter().collect();
            let tail: String = chars[chars.len() - tail..].iter().collect();
            format!("{}…{}", head.trim_end(), tail.trim_start())
        }
        _ => text.to_string(),
    }
}

/// The chips of the left or right row, one line per pin, aligned towards
/// the board.
struct Side {
    rows: Vec<Line>,
    pads: Vec<bool>,
    area: usize,
}

impl Side {
    /// Aligns the n-th label of every pin in one column, or flows the chips
    /// of each pin next to each other when `packed`.
    fn new(cells: &Cells, edge: Edge, packed: bool) -> Self {
        let left = edge == Edge::Left;
        let pins = cells.pinout.row(edge);
        let chips: Vec<Vec<Option<Chip>>> = pins.iter().map(|pin| cells.pin(pin)).collect();

        let columns = chips.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> = (0..columns)
            .map(|n| {
                chips
                    .iter()
                    .filter_map(|pin| pin.get(n).cloned().flatten())
                    .map(|chip| chip.width())
                    .max()
                    .unwrap_or(0)
            })
            .collect();

        let mut rows: Vec<Line> = chips
            .into_iter()
            .map(|pin| {
                let mut cells: Vec<Line> = if packed {
                    pin.into_iter().flatten().map(|chip| chip.line(0)).collect()
                } else {
                    // Chips fill their column; columns without any chip take
                    // no space
                    let mut pin = pin.into_iter();
                    widths
                        .iter()
                        .map(|&w| (w, pin.next().flatten()))
                        .filter(|(w, _)| *w > 0)
                        .map(|(w, chip)| match chip {
                            Some(chip) => chip.line(w),
                            None => {
                                let mut blank = Line::default();
                                blank.pad(w);
                                blank
                            }
                        })
                        .collect()
                };
                if left {
                    cells.reverse();
                }
                let mut line = Line::default();
                for (n, cell) in cells.into_iter().enumerate() {
                    if n > 0 {
                        line.pad(1);
                    }
                    line.append(cell);
                }
                line
            })
            .collect();

        let area = width(&rows);
        for row in &mut rows {
            if left {
                let mut aligned = Line::default();
                aligned.pad(area - row.width);
                aligned.append(std::mem::take(row));
                *row = aligned;
            } else {
                row.pad_to(area);
            }
        }
        Self {
            rows,
            pads: pins.iter().map(|pin| !pin.is_empty()).collect(),
            area,
        }
    }
}

/// The chips of the top or bottom row, stacked vertically per pin.
struct Stack {
    columns: Vec<Column>,
}

struct Column {
    /// Chips by distance from the board; `None` leaves the level empty.
    chips: Vec<Option<Chip>>,
    width: usize,
    pad: bool,
}

impl Stack {
    fn new(cells: &Cells, edge: Edge) -> Self {
        let columns = cells
            .pinout
            .row(edge)
            .iter()
            .map(|pin| {
                let chips = cells.pin(pin);
                Column {
                    width: chips
                        .iter()
                        .flatten()
                        .map(Chip::width)
                        .max()
                        .unwrap_or(0)
                        .max(1),
                    chips,
                    pad: !pin.is_empty(),
                }
            })
            .collect();
        Self { columns }
    }

    fn width(&self) -> usize {
        self.columns.iter().map(|c| c.width).sum::<usize>() + self.columns.len().saturating_sub(1)
    }

    fn depth(&self) -> usize {
        self.columns
            .iter()
            .map(|c| c.chips.len())
            .max()
            .unwrap_or(0)
    }

    /// Positions of the pads, relative to the start of the stack.
    fn pad_positions(&self) -> Vec<usize> {
        let mut x = 0;
        let mut positions = Vec::new();
        for column in &self.columns {
            if column.pad {
                positions.push(x + (column.width - 1) / 2);
            }
            x += column.width + 1;
        }
        positions
    }

    /// The `level`th chip of every column, level 0 being next to the board.
    fn line(&self, level: usize) -> Line {
        let mut line = Line::default();
        for (n, column) in self.columns.iter().enumerate() {
            if n > 0 {
                line.pad(1);
            }
            match column.chips.get(level).cloned().flatten() {
                Some(chip) => line.append(chip.line(column.width)),
                None => line.pad(column.width),
            }
        }
        line
    }
}

/// Draws the board with a pad for every pin and the chips around it.
fn render_board(
    left: &Side,
    right: &Side,
    top: &Stack,
    bottom: &Stack,
    label: &str,
    compact: bool,
) -> Vec<Line> {
    let label_width = label.chars().count();
    let inner = [
        if label.is_empty() { 3 } else { label_width + 6 },
        top.width() + 2,
        bottom.width() + 2,
    ]
    .into_iter()
    .max()
    .unwrap_or(3);
    let count = left.rows.len().max(right.rows.len());
    let left_width = if left.area > 0 { left.area + 1 } else { 0 };
    let edge = Style::board(BOARD_EDGE, false);

    // Body lines: Some(pin index) or None for the gap between pins
    let mut body = Vec::new();
    for i in 0..count {
        if i > 0 && !compact {
            body.push(None);
        }
        body.push(Some(i));
    }
    let middle = body.len().saturating_sub(1) / 2;

    let stack_line = |stack: &Stack, level: usize| {
        let mut line = Line::default();
        line.pad(left_width + 1 + (inner - stack.width()) / 2);
        line.append(stack.line(level));
        line
    };
    let border = |stack: &Stack, corners: (&str, &str)| {
        let start = (inner - stack.width()) / 2;
        let pads: Vec<usize> = stack.pad_positions().iter().map(|p| p + start).collect();
        // The edges share the board background so the body reads as one block
        let mut line = Line::default();
        line.pad(left_width);
        line.push(corners.0, edge);
        for x in 0..inner {
            if pads.contains(&x) {
                line.push("●", Style::board(PAD, true));
            } else {
                line.push("─", edge);
            }
        }
        line.push(corners.1, edge);
        line
    };

    let mut lines = Vec::new();
    for level in (0..top.depth()).rev() {
        lines.push(stack_line(top, level));
    }
    lines.push(border(top, ("╭", "╮")));

    for (n, pin) in body.iter().enumerate() {
        let mut line = Line::default();
        match pin.filter(|i| *i < left.rows.len()) {
            Some(i) if left_width > 0 => {
                line.append(left.rows[i].clone());
                line.pad(1);
            }
            _ => line.pad(left_width),
        }

        let pad = |side: &Side, line: &mut Line| {
            if pin.is_some_and(|i| side.pads.get(i).copied().unwrap_or(false)) {
                line.push("●", Style::board(PAD, true));
            } else {
                line.push(" ", Style::board(BOARD, false));
            }
        };
        line.push("│", edge);
        pad(left, &mut line);
        let text = if n == middle { label } else { "" };
        let space = inner - 2 - text.chars().count();
        line.push(" ".repeat(space / 2), Style::board(BOARD, false));
        line.push(text, Style::board(Rgb(235, 235, 235), true));
        line.push(" ".repeat(space - space / 2), Style::board(BOARD, false));
        pad(right, &mut line);
        line.push("│", edge);

        if let Some(i) = pin.filter(|i| *i < right.rows.len() && right.area > 0) {
            line.pad(1);
            line.append(right.rows[i].clone());
        }
        lines.push(line);
    }

    lines.push(border(bottom, ("╰", "╯")));
    for level in 0..bottom.depth() {
        lines.push(stack_line(bottom, level));
    }
    lines
}

fn legend_entry(kind: &ResolvedType) -> Line {
    let mut line = Line::default();
    line.push("  ", Style::chip(kind));
    line.push(format!(" {}", kind.label), Style::default());
    line
}

fn legend_box(types: &[ResolvedType]) -> Vec<Line> {
    if types.is_empty() {
        return Vec::new();
    }
    let entries: Vec<Line> = types.iter().map(legend_entry).collect();
    let inner = width(&entries) + 2;
    let border = Style::fg(FRAME);

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

/// Surrounds the content with a rounded border, the title set into its top
/// edge.
fn frame(title: Option<&str>, content: Vec<Line>) -> Vec<Line> {
    let border = Style::fg(FRAME);
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

/// Parses the CSS colors SVG accepts: names and `#rgb` / `#rrggbb`.
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

    fn pinout() -> Pinout {
        Pinout::from_yaml_str(
            r#"
title: MCU
width: 4
height: 2
pins:
  left:
    - [ "GPIO5:gpio", "MISO:spi" ]
    - [ "GPIO6:gpio" ]
  right:
    - [ "5V:power" ]
    - [ "GND:gnd" ]
  bottom:
    -
    - [ "SWDIO:gpio", "D:gpio" ]
"#,
        )
        .unwrap()
    }

    fn plain(options: TermOptions) -> String {
        let options = TermOptions {
            color: false,
            ..options
        };
        render_terminal(&pinout(), &options).unwrap()
    }

    #[test]
    fn renders_board_with_legend() {
        let out = plain(TermOptions::default());
        let expected = "\
╭─ MCU ─────────────────────────────────────────────────────╮
│                                                           │
│                 ╭───────────────╮          ╭───────────╮  │
│   MISO   GPIO5  │●             ●│  5V      │    GPIO   │  │
│                 │      MCU      │          │    Ground │  │
│          GPIO6  │●             ●│  GND     │    Power  │  │
│                 ╰──────●────────╯          │    SPI    │  │
│                      SWDIO                 ╰───────────╯  │
│                      D                                    │
│                                                           │
╰───────────────────────────────────────────────────────────╯
";
        assert_eq!(out, expected, "\n{out}");
    }

    #[test]
    fn back_side_swaps_left_and_right() {
        let out = plain(TermOptions {
            back: true,
            compact: true,
            ..TermOptions::default()
        });
        assert!(out.starts_with("╭─ MCU (back) "), "\n{out}");
        assert!(out.contains(" 5V   │●"), "\n{out}");
    }

    #[test]
    fn type_filter_hides_labels_and_rejects_unknown_types() {
        let out = plain(TermOptions {
            types: Some(vec!["gpio".to_string()]),
            ..TermOptions::default()
        });
        assert!(!out.contains("MISO") && !out.contains("5V"), "\n{out}");
        assert!(out.contains("GPIO5"));

        let options = TermOptions {
            types: Some(vec!["nope".to_string()]),
            ..TermOptions::default()
        };
        assert!(render_terminal(&pinout(), &options).is_err());
    }

    #[test]
    fn prints_notes_only_when_asked() {
        let mut pinout = pinout();
        pinout.notes = vec![crate::model::Note {
            title: Some("Features".to_string()),
            lines: vec!["WiFi".to_string()],
        }];
        let options = TermOptions {
            color: false,
            ..TermOptions::default()
        };
        assert!(!render_terminal(&pinout, &options).unwrap().contains("WiFi"));

        let options = TermOptions {
            notes: true,
            ..options
        };
        let out = render_terminal(&pinout, &options).unwrap();
        assert!(out.ends_with("╯\n\nFeatures\nWiFi\n"), "\n{out}");
    }

    #[test]
    fn aligns_label_columns_and_shortens_to_fit() {
        let pinout = Pinout::from_yaml_str(
            r#"
title: T
pins:
  right:
    - [ "GPIO1:gpio", "", "SDA:i2c" ]
    - [ "GPIO10:gpio", "A0:analog", "UART0 TX:uart" ]
"#,
        )
        .unwrap();
        let options = TermOptions {
            color: false,
            compact: true,
            ..TermOptions::default()
        };
        let out = render_terminal(&pinout, &options).unwrap();
        assert!(out.contains("●│  GPIO1         SDA          │"), "\n{out}");
        // Chips fill their column
        assert!(out.contains(" GPIO1  "), "\n{out}");
        assert!(out.contains("●│  GPIO10   A0   UART0 TX     │"), "\n{out}");

        let packed = render_terminal(
            &pinout,
            &TermOptions {
                packed: true,
                ..options.clone()
            },
        )
        .unwrap();
        assert!(packed.contains("●│  GPIO1   SDA "), "\n{packed}");

        // A column that is empty on every pin is skipped without shifting
        // the labels after it
        let skipped = Pinout::from_yaml_str(
            r#"
pins:
  left:
    - [ "PB5:gpio", "", "ADC0:analog" ]
    - [ "PB3:gpio", "", "MISO:spi" ]
"#,
        )
        .unwrap();
        let aligned = render_terminal(&skipped, &options).unwrap();
        assert!(aligned.contains(" ADC0   PB5  │●"), "\n{aligned}");
        assert!(aligned.contains(" MISO   PB3  │●"), "\n{aligned}");

        // Too narrow: labels are cut, the legend stays beside the board
        let wide = width_of(&out);
        let narrow = render_terminal(
            &pinout,
            &TermOptions {
                max_width: Some(wide - 4),
                ..options
            },
        )
        .unwrap();
        assert!(width_of(&narrow) <= wide - 4, "\n{narrow}");
        assert!(narrow.contains(" UA…TX "), "\n{narrow}");
        assert!(narrow.contains("│    GPIO   │"), "\n{narrow}");
    }

    #[test]
    fn shortens_from_the_middle() {
        assert_eq!(shorten("GPIO36", Some(5)), "GP…36");
        assert_eq!(shorten("EMAC TXD2", Some(6)), "EMA…D2");
        assert_eq!(shorten("GPIO5", Some(5)), "GPIO5");
        assert_eq!(shorten("GPIO5", None), "GPIO5");
    }

    fn width_of(out: &str) -> usize {
        out.lines().map(|l| l.chars().count()).max().unwrap_or(0)
    }

    #[test]
    fn parses_named_and_hex_colors() {
        assert_eq!(parse_color("DeepSkyBlue"), Some(Rgb(0, 191, 255)));
        assert_eq!(parse_color("#f80"), Some(Rgb(255, 136, 0)));
        assert_eq!(parse_color("#0"), None);
    }
}
