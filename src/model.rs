//! Board descriptions in the pinoutleaf format.
//!
//! A board has up to four rows of pins (`left`, `right`, `top`, `bottom`) on
//! the standard 0.1" raster. Every pin has a list of labels written as
//! `text:type`, where the type picks the colors and legend entry of the label.
//! The same model is read from and written to YAML or JSON, and the renderers
//! in [`crate::renderer`] draw it as SVG or terminal output.
//!
//! ```yaml
//! title: "ESP32 C3 Super Mini"
//! width: 7
//! height: 8
//! pins:
//!   left:
//!     - [ "GPIO5:gpio", "A5:analog", "MISO:spi" ]
//!     - [ "GPIO6:gpio", "MOSI:spi" ]
//!   right:
//!     - [ "5V:power" ]
//!     - [ "GND:gnd" ]
//! types:
//!   motor:
//!     label: Output
//!     bgcolor: "#439ED6"
//! notes:
//!   - title: Features
//!     lines: [ "WiFi 2.4GHz", "Bluetooth 5 LE" ]
//! ```

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PinoutError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] crate::parser::csv::ParserError),

    #[error("Invalid pinout: {0}")]
    Invalid(String),

    #[error("Unsupported file type: {0}")]
    UnsupportedFile(String),
}

/// A board and the labels of its pins.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pinout {
    pub title: String,
    /// Board width in pins; defaults to the longest of the top and bottom rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<usize>,
    /// Board height in pins; defaults to the longest of the left and right rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<usize>,
    #[serde(skip_serializing_if = "Images::is_empty")]
    pub image: Images,
    /// Custom types, or overrides of the built-in ones.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub types: BTreeMap<String, TypeStyle>,
    /// Moves a row of pins inwards by this many pins.
    #[serde(skip_serializing_if = "Offsets::is_zero")]
    pub offsets: Offsets,
    pub pins: Pins,
    /// Free text blocks such as feature lists or warnings. Not part of
    /// pinoutleaf, which ignores unknown fields.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Note {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pins {
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "pin_row")]
    pub left: Vec<Pin>,
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "pin_row")]
    pub right: Vec<Pin>,
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "pin_row")]
    pub top: Vec<Pin>,
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "pin_row")]
    pub bottom: Vec<Pin>,
}

/// The labels of one pin, ordered from the pin outwards. Empty for unused
/// positions.
pub type Pin = Vec<Label>;

/// Accepts `null` for a whole row or a single pin, as pinoutleaf does.
fn pin_row<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Pin>, D::Error> {
    let row: Option<Vec<Option<Pin>>> = Option::deserialize(deserializer)?;
    Ok(row
        .unwrap_or_default()
        .into_iter()
        .map(Option::unwrap_or_default)
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

    pub fn name(self) -> &'static str {
        match self {
            Edge::Left => "left",
            Edge::Right => "right",
            Edge::Top => "top",
            Edge::Bottom => "bottom",
        }
    }
}

/// One label of a pin, written as `text` or `text:type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub text: String,
    pub kind: Option<String>,
}

impl Label {
    pub fn new(text: impl Into<String>, kind: Option<&str>) -> Self {
        Self {
            text: text.into(),
            kind: kind.map(String::from),
        }
    }

    /// Splits off the type after the last colon. Suffixes that cannot be a
    /// type name stay part of the text, so `ADC1:0` is a label without type.
    pub fn parse(value: &str) -> Self {
        match value.rsplit_once(':') {
            Some((text, kind)) if is_type_name(kind) => Self::new(text, Some(kind)),
            _ => Self::new(value, None),
        }
    }
}

fn is_type_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            Some(kind) => write!(f, "{}:{}", self.text, kind),
            None => f.write_str(&self.text),
        }
    }
}

impl Serialize for Label {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Label {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Unquoted YAML pin numbers arrive as numbers
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Text(String),
            Integer(i64),
            Float(f64),
        }
        Ok(match Raw::deserialize(deserializer)? {
            Raw::Text(text) => Label::parse(&text),
            Raw::Integer(n) => Label::new(n.to_string(), None),
            Raw::Float(n) => Label::new(n.to_string(), None),
        })
    }
}

/// Colors and legend text of a label type. Unset fields fall back to the
/// built-in type of the same name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TypeStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bgcolor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fgcolor: Option<String>,
}

/// A type with every field filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedType {
    pub name: String,
    pub label: String,
    pub bgcolor: String,
    pub fgcolor: String,
}

/// The type used for labels without one.
pub const DEFAULT_TYPE: &str = "default";

/// Built-in types: name, legend label, background and text color.
const BUILTIN_TYPES: &[(&str, &str, &str, &str)] = &[
    (DEFAULT_TYPE, "Pin", "#ffffff", "#000000"),
    ("gpio", "GPIO", "#79bc3c", "#ffffff"),
    ("power", "Power", "#cc322d", "#ffffff"),
    ("gnd", "Ground", "#333333", "#ffffff"),
    ("i2c", "I2C", "#41b28c", "#ffffff"),
    ("uart", "UART", "#637181", "#ffffff"),
    ("spi", "SPI", "#775ee8", "#ffffff"),
    ("analog", "Analog", "#e38022", "#ffffff"),
];

pub fn is_builtin_type(name: &str) -> bool {
    BUILTIN_TYPES.iter().any(|(n, ..)| *n == name)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Images {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub front: Option<BoardImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back: Option<BoardImage>,
}

impl Images {
    fn is_empty(&self) -> bool {
        self.front.is_none() && self.back.is_none()
    }
}

/// A PNG or JPEG photo drawn instead of the plain board. It is stretched to the
/// board outline; the edge offsets (in 1/100 mm, negative moves outwards)
/// correct the fit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BoardImage {
    pub src: String,
    #[serde(skip_serializing_if = "is_zero")]
    pub top: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub left: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub right: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub bottom: f64,
    #[serde(skip_serializing_if = "is_default_opacity")]
    pub opacity: f64,
    #[serde(skip_serializing_if = "is_true")]
    pub grayscale: bool,
}

impl Default for BoardImage {
    fn default() -> Self {
        Self {
            src: String::new(),
            top: 0.0,
            left: 0.0,
            right: 0.0,
            bottom: 0.0,
            opacity: 0.5,
            grayscale: true,
        }
    }
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

fn is_default_opacity(value: &f64) -> bool {
    *value == 0.5
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Offsets {
    pub left: usize,
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
}

impl Offsets {
    fn is_zero(&self) -> bool {
        *self == Self::default()
    }
}

impl Pinout {
    pub fn from_yaml_str(input: &str) -> Result<Self, PinoutError> {
        Self::checked(serde_yaml_ng::from_str(input)?)
    }

    pub fn from_json_str(input: &str) -> Result<Self, PinoutError> {
        Self::checked(serde_json::from_str(input)?)
    }

    /// Reads a `.yaml`, `.yml` or `.json` file.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, PinoutError> {
        let path = path.as_ref();
        let input = std::fs::read_to_string(path)?;
        match extension(path).as_str() {
            "yaml" | "yml" => Self::from_yaml_str(&input),
            "json" => Self::from_json_str(&input),
            _ => Err(PinoutError::UnsupportedFile(path.display().to_string())),
        }
    }

    pub fn to_yaml(&self) -> Result<String, PinoutError> {
        Ok(serde_yaml_ng::to_string(self)?)
    }

    pub fn to_json(&self) -> Result<String, PinoutError> {
        Ok(serde_json::to_string_pretty(self)? + "\n")
    }

    fn checked(mut pinout: Self) -> Result<Self, PinoutError> {
        pinout.normalize();
        pinout.validate()?;
        Ok(pinout)
    }

    /// Folds labels with an unknown type back into their text, so that
    /// `HSPI:CLK` reads as one label unless a `CLK` type exists.
    pub fn normalize(&mut self) {
        let known: HashSet<String> = BUILTIN_TYPES
            .iter()
            .map(|(name, ..)| name.to_string())
            .chain(self.types.keys().cloned())
            .collect();
        for edge in Edge::ALL {
            for label in self.pins.row_mut(edge).iter_mut().flatten() {
                if label
                    .kind
                    .as_ref()
                    .is_some_and(|kind| !known.contains(kind))
                {
                    *label = Label::new(label.to_string(), None);
                }
            }
        }
    }

    pub fn validate(&self) -> Result<(), PinoutError> {
        let (width, height) = self.size();
        for edge in Edge::ALL {
            let (count, limit, offset) = match edge {
                Edge::Left => (self.pins.left.len(), height, self.offsets.left),
                Edge::Right => (self.pins.right.len(), height, self.offsets.right),
                Edge::Top => (self.pins.top.len(), width, self.offsets.top),
                Edge::Bottom => (self.pins.bottom.len(), width, self.offsets.bottom),
            };
            if count > limit {
                return Err(PinoutError::Invalid(format!(
                    "{} row has {count} pins but the board is only {limit} pins long",
                    edge.name()
                )));
            }
            let across = if matches!(edge, Edge::Left | Edge::Right) {
                width
            } else {
                height
            };
            if offset >= across {
                return Err(PinoutError::Invalid(format!(
                    "{} offset {offset} moves the row off the {across} pin board",
                    edge.name()
                )));
            }
        }
        Ok(())
    }

    /// Board size in pins as `(width, height)`.
    pub fn size(&self) -> (usize, usize) {
        let width = self
            .width
            .unwrap_or_else(|| self.pins.top.len().max(self.pins.bottom.len()).max(2));
        let height = self
            .height
            .unwrap_or_else(|| self.pins.left.len().max(self.pins.right.len()).max(2));
        (width, height)
    }

    /// The pins along an edge, padded with empty pins to the board size.
    pub fn row(&self, edge: Edge) -> Vec<Pin> {
        let (width, height) = self.size();
        let length = match edge {
            Edge::Left | Edge::Right => height,
            Edge::Top | Edge::Bottom => width,
        };
        let mut row = self.pins.row(edge).to_vec();
        row.resize(length.max(row.len()), Vec::new());
        row
    }

    /// The board seen from the back: left and right swap, top and bottom
    /// reverse and the back image becomes the visible one.
    pub fn flipped(&self) -> Self {
        let (width, height) = self.size();
        let mut flipped = self.clone();
        flipped.width = Some(width);
        flipped.height = Some(height);
        flipped.pins.left = self.row(Edge::Right);
        flipped.pins.right = self.row(Edge::Left);
        flipped.pins.top = self.row(Edge::Top).into_iter().rev().collect();
        flipped.pins.bottom = self.row(Edge::Bottom).into_iter().rev().collect();
        std::mem::swap(&mut flipped.offsets.left, &mut flipped.offsets.right);
        std::mem::swap(&mut flipped.image.front, &mut flipped.image.back);
        flipped
    }

    /// Colors and legend text for a label type, merging custom settings over
    /// the built-in defaults. Unknown types resolve to the default type.
    pub fn resolve_type(&self, kind: Option<&str>) -> ResolvedType {
        let name = kind
            .filter(|k| is_builtin_type(k) || self.types.contains_key(*k))
            .unwrap_or(DEFAULT_TYPE);
        let (label, bgcolor, fgcolor) = BUILTIN_TYPES
            .iter()
            .find(|(n, ..)| *n == name)
            .map(|(_, label, bg, fg)| (label.to_string(), bg.to_string(), fg.to_string()))
            .unwrap_or_else(|| (name.to_string(), "#ffffff".into(), "#000000".into()));
        let custom = self.types.get(name);
        ResolvedType {
            name: name.to_string(),
            label: custom.and_then(|t| t.label.clone()).unwrap_or(label),
            bgcolor: custom.and_then(|t| t.bgcolor.clone()).unwrap_or(bgcolor),
            fgcolor: custom.and_then(|t| t.fgcolor.clone()).unwrap_or(fgcolor),
        }
    }

    /// Every type used by a label, sorted by legend text. Blank labels only
    /// keep a column free and do not count.
    pub fn used_types(&self) -> Vec<ResolvedType> {
        let mut used: Vec<ResolvedType> = Vec::new();
        for edge in Edge::ALL {
            for label in self.pins.row(edge).iter().flatten() {
                if label.text.is_empty() {
                    continue;
                }
                let resolved = self.resolve_type(label.kind.as_deref());
                if !used.iter().any(|t| t.name == resolved.name) {
                    used.push(resolved);
                }
            }
        }
        used.sort_by(|a, b| a.label.cmp(&b.label));
        used
    }
}

impl Pins {
    pub fn row(&self, edge: Edge) -> &[Pin] {
        match edge {
            Edge::Left => &self.left,
            Edge::Right => &self.right,
            Edge::Top => &self.top,
            Edge::Bottom => &self.bottom,
        }
    }

    pub fn row_mut(&mut self, edge: Edge) -> &mut Vec<Pin> {
        match edge {
            Edge::Left => &mut self.left,
            Edge::Right => &mut self.right,
            Edge::Top => &mut self.top,
            Edge::Bottom => &mut self.bottom,
        }
    }
}

pub(crate) fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"
title: "Demo"
width: 4
height: 3
types:
  motor:
    label: Output
    bgcolor: "#439ED6"
pins:
  left:
    - [ "GPIO5:gpio", "A5:analog", "MISO:spi" ]
    -
    - [ 3, "GND:gnd" ]
  right:
    - [ "5V:power" ]
    - [ "1A:motor", "ADC1:0", "HSPI:CLK" ]
  bottom:
    - [ "SWDIO:debug" ]
notes:
  - title: Features
    lines: [ "WiFi", "BLE" ]
"##;

    #[test]
    fn parses_pinoutleaf_yaml() {
        let pinout = Pinout::from_yaml_str(SAMPLE).unwrap();
        assert_eq!(pinout.title, "Demo");
        assert_eq!(pinout.pins.left[0][1], Label::new("A5", Some("analog")));
        assert!(pinout.pins.left[1].is_empty());
        assert_eq!(pinout.pins.left[2][0], Label::new("3", None));
        // Types that do not exist stay part of the text
        assert_eq!(pinout.pins.right[1][1], Label::new("ADC1:0", None));
        assert_eq!(pinout.pins.right[1][2], Label::new("HSPI:CLK", None));
        assert_eq!(pinout.pins.bottom[0][0], Label::new("SWDIO:debug", None));
        assert_eq!(pinout.notes[0].title.as_deref(), Some("Features"));
        assert_eq!(pinout.notes[0].lines, ["WiFi", "BLE"]);
    }

    #[test]
    fn yaml_and_json_round_trip() {
        let pinout = Pinout::from_yaml_str(SAMPLE).unwrap();
        assert_eq!(
            Pinout::from_json_str(&pinout.to_json().unwrap()).unwrap(),
            pinout
        );
        assert_eq!(
            Pinout::from_yaml_str(&pinout.to_yaml().unwrap()).unwrap(),
            pinout
        );
    }

    #[test]
    fn resolves_custom_and_builtin_types() {
        let pinout = Pinout::from_yaml_str(SAMPLE).unwrap();
        let motor = pinout.resolve_type(Some("motor"));
        assert_eq!(motor.label, "Output");
        assert_eq!(motor.bgcolor, "#439ED6");
        assert_eq!(motor.fgcolor, "#000000");
        assert_eq!(pinout.resolve_type(Some("nope")).name, DEFAULT_TYPE);
        let labels: Vec<String> = pinout.used_types().into_iter().map(|t| t.label).collect();
        assert_eq!(
            labels,
            ["Analog", "GPIO", "Ground", "Output", "Pin", "Power", "SPI"]
        );
    }

    #[test]
    fn flips_board() {
        let pinout = Pinout::from_yaml_str(SAMPLE).unwrap();
        let back = pinout.flipped();
        assert_eq!(back.pins.left[0][0].text, "5V");
        assert_eq!(back.pins.right[0][0].text, "GPIO5");
        assert_eq!(back.pins.bottom.len(), 4);
        assert_eq!(back.pins.bottom[3][0].text, "SWDIO:debug");
    }

    #[test]
    fn example_files_load() {
        for file in ["ATtiny85.yaml", "ESP32-MAXIO.yaml"] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
            let pinout = Pinout::from_path(&path).unwrap_or_else(|e| panic!("{file}: {e}"));
            // A type name that does not exist would leave a colon in the text
            for label in Edge::ALL.iter().flat_map(|e| pinout.row(*e)).flatten() {
                assert!(
                    !label.text.contains(':') || label.kind.is_some(),
                    "{file}: {label}"
                );
            }
        }
    }

    #[test]
    fn infers_size_and_rejects_overflowing_rows() {
        let pinout = Pinout::from_yaml_str("pins: { left: [[A], [B], [C]] }").unwrap();
        assert_eq!(pinout.size(), (2, 3));

        let error = Pinout::from_yaml_str("height: 1\npins: { left: [[A], [B]] }").unwrap_err();
        assert!(error.to_string().contains("left row has 2 pins"), "{error}");
        assert!(Pinout::from_yaml_str("titel: typo").is_err());
    }
}
