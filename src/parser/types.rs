use serde::{Deserialize, Serialize};
use std::fmt;

/// Represents the phase of the command
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Setup,
    Draw,
}

/// Represents a parsed command from the CSV file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    // Setup Phase Commands
    Labels {
        default: String,
        pin_type: Option<String>,
        group: Option<String>,
        labels: Vec<String>,
    },
    BorderColor {
        default: String,
        pin_type: Option<String>,
        group: Option<String>,
        colors: Vec<String>,
    },
    BorderWidth {
        width: u32,
    },
    BorderOpacity {
        opacity: f32,
    },
    FillColor {
        default: String,
        pin_type: Option<String>,
        group: Option<String>,
        colors: Vec<String>,
    },
    Opacity {
        default: f32,
        pin_type: Option<f32>,
        group: Option<f32>,
        opacities: Vec<f32>,
    },
    Font {
        default: String,
        pin_type: Option<String>,
        group: Option<String>,
        fonts: Vec<String>,
    },
    FontSize {
        default: f32,
        pin_type: Option<f32>,
        group: Option<f32>,
        sizes: Vec<f32>,
    },
    FontColor {
        default: String,
        pin_type: Option<String>,
        group: Option<String>,
        colors: Vec<String>,
    },
    FontOutline {
        default: String,
        pin_type: Option<String>,
        group: Option<String>,
        colors: Vec<String>,
    },
    FontOutlineThickness {
        default: f32,
        pin_type: Option<f32>,
        group: Option<f32>,
        thickness: Vec<f32>,
    },
    FontSlant {
        default: FontSlant,
        pin_type: Option<FontSlant>,
        group: Option<FontSlant>,
        slants: Vec<FontSlant>,
    },
    FontBold {
        default: FontBoldness,
        pin_type: Option<FontBoldness>,
        group: Option<FontBoldness>,
        boldness: Vec<FontBoldness>,
    },
    FontStretch {
        default: FontStretch,
        pin_type: Option<FontStretch>,
        group: Option<FontStretch>,
        stretches: Vec<FontStretch>,
    },
    Type {
        pin_type: PinType,
        color: String,
        opacity: f32,
    },
    Wire {
        wire_type: WireType,
        color: String,
        opacity: f32,
        thickness: f32,
    },
    Group {
        name: String,
        color: String,
        opacity: f32,
    },
    BoxTheme {
        name: String,
        border_color: String,
        border_opacity: f32,
        fill_color: String,
        fill_opacity: f32,
        line_width: f32,
        box_width: f32,
        box_height: f32,
        box_cr_x: f32,
        box_cr_y: f32,
        box_skew: f32,
        box_skew_offset: f32,
    },
    TextFont {
        theme_name: String,
        font: String,
        size: f32,
        outline_color: String,
        color: String,
        slant: FontSlant,
        bold: FontBoldness,
        stretch: FontStretch,
    },
    Page {
        page_name: String,
    },
    Dpi {
        dpi: u32,
    },
    Draw, // Starts the Draw phase

    // Draw Phase Commands
    GoogleFont {
        _link: String,
    },
    Image {
        name: String,
        x: Option<f32>,
        y: Option<f32>,
        w: Option<f32>,
        h: Option<f32>,
        cx: Option<f32>,
        cy: Option<f32>,
        cw: Option<f32>,
        ch: Option<f32>,
        rot: Option<f32>,
    },
    Icon {
        name: String,
        x: Option<f32>,
        y: Option<f32>,
        w: Option<f32>,
        h: Option<f32>,
        rot: Option<f32>,
    },
    Anchor {
        x: f32,
        y: f32,
    },
    PinSet {
        side: Side,
        packed: bool,
        justify_x: JustifyX,
        justify_y: JustifyY,
        line_step: f32,
        pin_width: f32,
        group_width: f32,
        leader_offset: f32,
        column_gap: f32,
        leader_h_step: f32,
    },
    Pin {
        wire: Option<WireType>,
        pin_type: Option<PinType>,
        group: Option<String>,
        attributes: Vec<String>,
    },
    PinText {
        wire: Option<WireType>,
        pin_type: Option<PinType>,
        pin_group: Option<String>,
        msg_theme: String,
        label: Option<String>,
        message: String,
    },
    Box {
        theme: String,
        x: f32,
        y: f32,
        box_width: Option<f32>,
        box_height: Option<f32>,
        x_justify: Option<JustifyX>,
        y_justify: Option<JustifyY>,
        message: Option<String>,
    },
    Message {
        x: Option<f32>,
        y: Option<f32>,
        line_step: Option<f32>,
        font: Option<String>,
        font_size: Option<f32>,
        x_justify: Option<JustifyX>,
        y_justify: Option<JustifyY>,
    },
    Text {
        edge_color: String,
        color: String,
        message: String,
        new_line: bool,
    },
    EndMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PinType {
    IO,
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireType {
    Digital,
    Pwm,
    Analog,
    HsAnalog,
    Power,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifyX {
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JustifyY {
    Top,
    Bottom,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSlant {
    Normal,
    Italic,
    Oblique,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontBoldness {
    Normal,
    Bold,
    Bolder,
    Lighter,
    Weight100,
    Weight200,
    Weight300,
    Weight400,
    Weight500,
    Weight600,
    Weight700,
    Weight800,
    Weight900,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStretch {
    Normal,
    Wider,
    Narrower,
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

impl fmt::Display for PinType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinType::IO => write!(f, "IO"),
            PinType::Input => write!(f, "INPUT"),
            PinType::Output => write!(f, "OUTPUT"),
        }
    }
}

impl fmt::Display for WireType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireType::Digital => write!(f, "DIGITAL"),
            WireType::Pwm => write!(f, "PWM"),
            WireType::Analog => write!(f, "ANALOG"),
            WireType::HsAnalog => write!(f, "HS-ANALOG"),
            WireType::Power => write!(f, "POWER"),
        }
    }
}

impl fmt::Display for FontSlant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontSlant::Normal => write!(f, "normal"),
            FontSlant::Italic => write!(f, "italic"),
            FontSlant::Oblique => write!(f, "oblique"),
        }
    }
}

impl fmt::Display for FontBoldness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontBoldness::Normal => write!(f, "normal"),
            FontBoldness::Bold => write!(f, "bold"),
            FontBoldness::Bolder => write!(f, "bolder"),
            FontBoldness::Lighter => write!(f, "lighter"),
            FontBoldness::Weight100 => write!(f, "100"),
            FontBoldness::Weight200 => write!(f, "200"),
            FontBoldness::Weight300 => write!(f, "300"),
            FontBoldness::Weight400 => write!(f, "400"),
            FontBoldness::Weight500 => write!(f, "500"),
            FontBoldness::Weight600 => write!(f, "600"),
            FontBoldness::Weight700 => write!(f, "700"),
            FontBoldness::Weight800 => write!(f, "800"),
            FontBoldness::Weight900 => write!(f, "900"),
        }
    }
}

impl fmt::Display for FontStretch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontStretch::Normal => write!(f, "normal"),
            FontStretch::Wider => write!(f, "wider"),
            FontStretch::Narrower => write!(f, "narrower"),
            FontStretch::UltraCondensed => write!(f, "ultra-condensed"),
            FontStretch::ExtraCondensed => write!(f, "extra-condensed"),
            FontStretch::Condensed => write!(f, "condensed"),
            FontStretch::SemiCondensed => write!(f, "semi-condensed"),
            FontStretch::SemiExpanded => write!(f, "semi-expanded"),
            FontStretch::Expanded => write!(f, "expanded"),
            FontStretch::ExtraExpanded => write!(f, "extra-expanded"),
            FontStretch::UltraExpanded => write!(f, "ultra-expanded"),
        }
    }
}
