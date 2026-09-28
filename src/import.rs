//! Converts the legacy CSV pinout descriptions into the [`Pinout`] model.
//!
//! The CSV format positions pin sets freely on a page and styles labels per
//! column, while the model describes one board with typed labels. The import
//! therefore takes the main pin sets of the board and guesses each label's
//! type from its name. Everything that does not fit is reported as a warning.

use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{is_builtin_type, Label, Note, Pin, Pinout, PinoutError, TypeStyle};
use crate::parser::csv::parse_csv_file;
use crate::parser::types::{Command, Side};

/// The imported board and what could not be carried over.
#[derive(Debug)]
pub struct Import {
    pub pinout: Pinout,
    pub warnings: Vec<String>,
}

pub fn from_csv_file(path: impl AsRef<Path>) -> Result<Import, PinoutError> {
    let path = path.as_ref().to_string_lossy().to_string();
    Ok(from_commands(&parse_csv_file(&path)?))
}

struct PinSet {
    side: Side,
    anchor: (f32, f32),
    line_step: f32,
    pins: Vec<Pin>,
}

/// A titled box on the page; its title heads the first message inside it.
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

pub fn from_commands(commands: &[Command]) -> Import {
    let mut sets: Vec<PinSet> = Vec::new();
    let mut anchor = (0.0, 0.0);
    let mut notes: Vec<Note> = Vec::new();
    let mut boxes: Vec<HeadingBox> = Vec::new();
    let mut warnings = Vec::new();
    let mut images = 0;
    let mut categories: Vec<Category> = Vec::new();
    let mut label = |text: &str, column: usize| {
        let category = Category::classify(text, column);
        if !categories.contains(&category) {
            categories.push(category);
        }
        Label::new(text, category.type_name())
    };

    for command in commands {
        match command {
            Command::Anchor { x, y } => anchor = (*x, *y),
            Command::PinSet {
                side, line_step, ..
            } => sets.push(PinSet {
                side: *side,
                anchor,
                line_step: *line_step,
                pins: Vec::new(),
            }),
            Command::Pin { attributes, .. } => {
                // Empty cells stay as blank labels so the CSV columns line up
                let mut cells: Vec<String> = attributes.iter().map(|a| clean(a)).collect();
                while cells.last().is_some_and(String::is_empty) {
                    cells.pop();
                }
                let pin = cells
                    .iter()
                    .enumerate()
                    .map(|(column, text)| match text.is_empty() {
                        true => Label::new("", None),
                        false => label(text, column),
                    })
                    .collect();
                if let Some(set) = sets.last_mut() {
                    set.pins.push(pin);
                }
            }
            Command::PinText {
                label: name,
                message,
                ..
            } => {
                let mut pin: Pin = name
                    .as_deref()
                    .map(clean)
                    .filter(|n| !n.is_empty())
                    .map(|n| label(&n, 0))
                    .into_iter()
                    .collect();
                if !clean(message).is_empty() {
                    pin.push(Label::new(clean(message), None));
                }
                if let Some(set) = sets.last_mut() {
                    set.pins.push(pin);
                }
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
            Command::Message { x, y, .. } => {
                let (x, y) = (x.unwrap_or(0.0), y.unwrap_or(0.0));
                let heading = boxes.iter().position(|b| b.contains(x, y));
                notes.push(Note {
                    title: heading.map(|i| boxes.remove(i).title),
                    lines: Vec::new(),
                });
            }
            Command::Text { message, .. } => {
                if notes.is_empty() {
                    notes.push(Note::default());
                }
                if let Some(note) = notes.last_mut() {
                    note.lines.push(clean(message));
                }
            }
            Command::Image { .. } | Command::Icon { .. } => images += 1,
            _ => {}
        }
    }

    if images > 0 {
        warnings.push(format!("skipped {images} images and icons"));
    }

    // The first message is the board title
    let title = notes
        .first()
        .and_then(|note| note.lines.iter().find(|l| !l.is_empty()).cloned());
    if notes.first().is_some_and(|note| note.lines.len() == 1) {
        notes.remove(0);
    }
    let mut pinout = Pinout {
        title: title.unwrap_or_default(),
        notes,
        ..Pinout::default()
    };

    // The board is the first left pin set and the right pin set on the same
    // height; the remaining sets are extra headers or keys.
    let left = sets.iter().position(|s| s.side == Side::Left);
    let right = left
        .and_then(|l| {
            sets.iter()
                .position(|s| s.side == Side::Right && (s.anchor.1 - sets[l].anchor.1).abs() < 1.0)
        })
        .or_else(|| sets.iter().position(|s| s.side == Side::Right));
    let top = sets.iter().position(|s| s.side == Side::Top);
    let bottom = sets.iter().position(|s| s.side == Side::Bottom);
    let used = [left, right, top, bottom];

    if let (Some(l), Some(r)) = (left, right) {
        // Keep the distance between the rows in proportion to the pin pitch
        let pitch = sets[l].line_step.max(1.0);
        let across = ((sets[r].anchor.0 - sets[l].anchor.0).abs() / pitch).round() as usize + 1;
        pinout.width = Some(across.max(2));
    }
    for (index, set) in sets.iter().enumerate() {
        if !used.contains(&Some(index)) {
            warnings.push(format!(
                "skipped {:?} pin set at ({}, {}) with {} pins, only one board is supported",
                set.side,
                set.anchor.0,
                set.anchor.1,
                set.pins.len()
            ));
        }
    }
    let take = |index: Option<usize>| index.map(|i| sets[i].pins.clone()).unwrap_or_default();
    pinout.pins.left = take(left);
    pinout.pins.right = take(right);
    pinout.pins.top = take(top);
    pinout.pins.bottom = take(bottom);

    let height = pinout.pins.left.len().max(pinout.pins.right.len());
    pinout.height = Some(height.max(2));
    if pinout.width.is_none() {
        pinout.width = Some(pinout.pins.top.len().max(pinout.pins.bottom.len()).max(2));
    }

    pinout.types = categories
        .iter()
        .filter_map(|c| {
            let name = c.type_name()?;
            (!is_builtin_type(name)).then(|| (name.to_string(), c.style()))
        })
        .collect::<BTreeMap<_, _>>();

    Import { pinout, warnings }
}

/// What a pin function is used for, guessed from datasheet naming.
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
    /// Type in the model; built-in names where one exists.
    fn type_name(self) -> Option<&'static str> {
        Some(match self {
            Category::Pin => return None,
            Category::Gpio => "gpio",
            Category::Analog => "analog",
            Category::Power => "power",
            Category::Ground => "gnd",
            Category::I2c => "i2c",
            Category::Spi => "spi",
            Category::Uart => "uart",
            Category::Touch => "touch",
            Category::Rtc => "rtc",
            Category::Control => "control",
            Category::Serial => "serial",
            Category::Pwm => "pwm",
            Category::Interrupt => "interrupt",
            Category::SdCard => "sd",
            Category::Ethernet => "ethernet",
            Category::Clock => "clock",
            Category::Debug => "debug",
            Category::Logic => "logic",
            Category::Other => "other",
        })
    }

    /// Style for the types that are not built in.
    fn style(self) -> TypeStyle {
        let (label, bgcolor, fgcolor) = match self {
            Category::Touch => ("Touch", "#ec407a", "#ffffff"),
            Category::Rtc => ("RTC", "#a1887f", "#000000"),
            Category::Control => ("Control", "#ffb300", "#000000"),
            Category::Serial => ("Serial (SERCOM)", "#ab47bc", "#ffffff"),
            Category::Pwm => ("PWM / Timer", "#fdd835", "#000000"),
            Category::Interrupt => ("Interrupt", "#00acc1", "#ffffff"),
            Category::SdCard => ("SD Card", "#3949ab", "#ffffff"),
            Category::Ethernet => ("Ethernet", "#1e88e5", "#ffffff"),
            Category::Clock => ("Clock", "#c0ca33", "#000000"),
            Category::Debug => ("Debug", "#4e342e", "#ffffff"),
            Category::Logic => ("Custom Logic", "#827717", "#ffffff"),
            _ => ("Other", "#9e9e9e", "#000000"),
        };
        TypeStyle {
            label: Some(label.to_string()),
            bgcolor: Some(bgcolor.to_string()),
            fgcolor: Some(fgcolor.to_string()),
        }
    }

    /// Guess the category from the function name. The first column holds the
    /// board's own pin names, which fall back to [`Category::Pin`].
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

#[cfg(test)]
mod tests {
    use super::*;

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
            ("VSPICLK", 6, Category::Spi),
            ("MISO", 1, Category::Spi),
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
    fn imports_esp32_example() {
        let import =
            from_csv_file(concat!(env!("CARGO_MANIFEST_DIR"), "/ESP32-MAXIO.csv")).unwrap();
        let pinout = import.pinout;
        assert_eq!(pinout.title, "ESP32-MAXIO");
        assert_eq!(pinout.size(), (11, 26));
        assert_eq!(
            pinout.pins.left[1][..3],
            [
                Label::new("I36", None),
                Label::new("SENSOR VP", Some("analog")),
                Label::new("GPIO36", Some("gpio")),
            ]
        );
        // The empty "Analog 2" cell keeps the RTC label in its column
        assert_eq!(pinout.pins.left[1][4], Label::new("", None));
        assert_eq!(pinout.pins.left[1][5].text, "RTC:00");
        assert_eq!(
            pinout.pins.left[0],
            [
                Label::new("3.3VP", Some("power")),
                Label::new("3.3V (~250ma) Switched Supply", None),
            ]
        );
        assert!(pinout.types.contains_key("touch"));
        let features = pinout
            .notes
            .iter()
            .find(|n| n.title.as_deref() == Some("Features"))
            .unwrap();
        assert_eq!(
            features.lines[0],
            "ESP32-MAXIO by Sakura Industries Limited"
        );
        assert_eq!(pinout.notes[0].title.as_deref(), Some("Battery Connector"));
        assert_eq!(pinout.notes[0].lines[0], "WARNING!");
        assert!(!pinout.types.contains_key("gpio"));
        // Battery header and key do not belong to the board
        assert_eq!(
            import
                .warnings
                .iter()
                .filter(|w| w.contains("pin set"))
                .count(),
            2
        );

        // Serialized labels read back with the same types
        let yaml = pinout.to_yaml().unwrap();
        assert_eq!(Pinout::from_yaml_str(&yaml).unwrap(), pinout);
    }
}
