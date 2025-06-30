use csv::{ReaderBuilder, StringRecord};
use thiserror::Error;

use super::types::{
    Command, FontBoldness, FontSlant, FontStretch, JustifyX, JustifyY, Phase, PinType, Side,
    WireType,
};

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("CSV error: {0}")]
    CsvError(#[from] csv::Error),

    #[error("Failed to parse command: {0}")]
    ParseError(String),

    #[error("Invalid phase for command")]
    InvalidPhase,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Parses a CSV file into a list of commands
pub fn parse_csv_file(path: &str) -> Result<Vec<Command>, ParserError> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)?;

    let mut commands = Vec::new();
    let mut phase = Phase::Setup;

    for result in reader.records() {
        let record = result?;

        if record.is_empty()
            || record
                .get(0)
                .map_or(true, |s| s.trim().is_empty() || s.trim().starts_with('#'))
        {
            continue;
        }

        let command_name = record.get(0).unwrap().trim().to_uppercase();

        // Check for phase transition
        if command_name == "DRAW" {
            phase = Phase::Draw;
            commands.push(Command::Draw);
            continue;
        }

        let command = parse_command(command_name, &record, phase)?;
        commands.push(command);
    }

    Ok(commands)
}

/// Parses a single command from a CSV record
fn parse_command(
    command_name: String,
    record: &StringRecord,
    phase: Phase,
) -> Result<Command, ParserError> {
    println!("handling command {}", command_name);
    match (command_name.as_str(), phase) {
        // Setup Phase Commands
        ("LABELS", Phase::Setup) => parse_label_command(record),
        ("BORDER COLOR", Phase::Setup) => parse_border_color_command(record),
        ("BORDER WIDTH", Phase::Setup) => parse_border_width_command(record),
        ("BORDER OPACITY", Phase::Setup) => parse_border_opacity_command(record),
        ("FILL COLOR", Phase::Setup) => parse_fill_color_command(record),
        ("OPACITY", Phase::Setup) => parse_opacity_command(record),
        ("FONT", Phase::Setup) => parse_font_command(record),
        ("FONT SIZE", Phase::Setup) => parse_font_size_command(record),
        ("FONT COLOR", Phase::Setup) => parse_font_color_command(record),
        ("FONT SLANT", Phase::Setup) => parse_font_slant_command(record),
        ("FONT BOLD", Phase::Setup) => parse_font_bold_command(record),
        ("FONT STRETCH", Phase::Setup) => parse_font_stretch_command(record),
        ("FONT OUTLINE", Phase::Setup) => parse_font_outline_command(record),
        ("FONT OUTLINE THICKNESS", Phase::Setup) => parse_font_outline_thickness_command(record),
        ("TYPE", Phase::Setup) => parse_type_command(record),
        ("WIRE", Phase::Setup) => parse_wire_command(record),
        ("GROUP", Phase::Setup) => parse_group_command(record),
        ("BOX", Phase::Setup) => parse_box_theme_command(record),
        ("TEXT FONT", Phase::Setup) => parse_text_font_command(record),
        ("PAGE", Phase::Setup) => parse_page_command(record),
        ("DPI", Phase::Setup) => parse_dpi_command(record),

        // Draw Phase Commands
        ("GOOGLEFONT", Phase::Draw) => parse_google_font_command(record),
        ("IMAGE", Phase::Draw) => parse_image_command(record),
        ("ICON", Phase::Draw) => parse_icon_command(record),
        ("ANCHOR", Phase::Draw) => parse_anchor_command(record),
        ("PINSET", Phase::Draw) => parse_pinset_command(record),
        ("PIN", Phase::Draw) => parse_pin_command(record),
        ("PINTEXT", Phase::Draw) => parse_pin_text_command(record),
        ("BOX", Phase::Draw) => parse_box_command(record),
        ("MESSAGE", Phase::Draw) => parse_message_command(record),
        ("TEXT", Phase::Draw) => parse_text_command(record),
        ("END MESSAGE", Phase::Draw) => Ok(Command::EndMessage),

        // Invalid phase for command
        _ => {
            println!("{}", command_name);
            Err(ParserError::InvalidPhase)
        }
    }
}

fn parse_font_outline_thickness_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT OUTLINE THICKNESS command requires at least a default value".to_string(),
        ));
    }

    let default = parse_f32(record.get(1).unwrap())?;
    let pin_type = record.get(2).map(|s| parse_f32(s).ok()).flatten();
    let group = record.get(3).map(|s| parse_f32(s).ok()).flatten();

    let mut thickness = Vec::new();
    for i in 4..record.len() {
        if let Some(thickness_str) = record.get(i) {
            if !thickness_str.is_empty() {
                let size = parse_f32(thickness_str)?;
                thickness.push(size);
            }
        }
    }

    Ok(Command::FontOutlineThickness {
        default,
        pin_type,
        group,
        thickness,
    })
}

fn parse_font_outline_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT OUTLINE command requires at least a default value".to_string(),
        ));
    }

    let default = record.get(1).unwrap().to_string();
    let pin_type = record.get(2).map(|s| s.to_string());
    let group = record.get(3).map(|s| s.to_string());

    let mut colors = Vec::new();
    for i in 4..record.len() {
        if let Some(color) = record.get(i) {
            colors.push(color.to_string());
        }
    }

    Ok(Command::FontOutline {
        default,
        pin_type,
        group,
        colors,
    })
}

fn parse_border_opacity_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "BORDER OPACITY command requires at least a default value".to_string(),
        ));
    }

    let opacity = record.get(1).unwrap().to_string();

    Ok(Command::BorderOpacity { opacity })
}

fn parse_border_width_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "BORDER WIDTH command requires at least a default value".to_string(),
        ));
    }

    let width = record.get(1).unwrap().to_string();

    Ok(Command::BorderWidth { width })
}

// Parse functions for each command type
fn parse_label_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "LABELS command requires at least a default value".to_string(),
        ));
    }

    let default = record.get(1).unwrap().to_string();
    let pin_type = record.get(2).map(|s| s.to_string());
    let group = record.get(3).map(|s| s.to_string());

    let mut labels = Vec::new();
    for i in 4..record.len() {
        if let Some(label) = record.get(i) {
            labels.push(label.to_string());
        }
    }

    Ok(Command::Labels {
        default,
        pin_type,
        group,
        labels,
    })
}

// Similar parse functions for other commands would follow...
// I'll include a few examples, but for brevity won't implement all of them

fn parse_border_color_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "BORDER COLOR command requires at least a default value".to_string(),
        ));
    }

    let default = record.get(1).unwrap().to_string();
    let pin_type = record.get(2).map(|s| s.to_string());
    let group = record.get(3).map(|s| s.to_string());

    let mut colors = Vec::new();
    for i in 4..record.len() {
        if let Some(color) = record.get(i) {
            colors.push(color.to_string());
        }
    }

    Ok(Command::BorderColor {
        default,
        pin_type,
        group,
        colors,
    })
}

fn parse_pin_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "PIN command requires at least one attribute".to_string(),
        ));
    }

    let wire = record.get(1).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            match s.to_uppercase().as_str() {
                "DIGITAL" => Some(WireType::Digital),
                "PWM" => Some(WireType::Pwm),
                "ANALOG" => Some(WireType::Analog),
                "HS-ANALOG" => Some(WireType::HsAnalog),
                "POWER" => Some(WireType::Power),
                _ => None,
            }
        }
    });

    let pin_type = record.get(2).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            match s.to_uppercase().as_str() {
                "IO" => Some(PinType::IO),
                "INPUT" => Some(PinType::Input),
                "OUTPUT" => Some(PinType::Output),
                _ => None,
            }
        }
    });

    let group = record.get(3).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    let mut attributes = Vec::new();
    for i in 4..record.len() {
        if let Some(attr) = record.get(i) {
            attributes.push(attr.to_string());
        }
    }

    Ok(Command::Pin {
        wire,
        pin_type,
        group,
        attributes,
    })
}

// Helper functions for parsing values
// Helper functions for parsing values
fn parse_f32(value: &str) -> Result<f32, ParserError> {
    // First, try to parse as f32 directly

    match value.parse::<f32>() {
        Ok(float_val) => Ok(float_val),
        Err(_) => {
            // If f32 parsing failed, try to parse as integer first
            match value.parse::<u32>() {
                Ok(int_val) => Ok(int_val as f32), // Convert the integer to f32
                Err(_) => {
                    // Try to handle any special formatting that might cause issues
                    let cleaned_value = value.trim().replace(",", "");
                    cleaned_value.parse::<f32>().map_err(|_| {
                        ParserError::ParseError(format!("Failed to parse float: {}", value))
                    })
                }
            }
        }
    }
}

fn parse_u32(value: &str) -> Result<u32, ParserError> {
    value
        .parse()
        .map_err(|_| ParserError::ParseError(format!("Failed to parse integer: {}", value)))
}

fn parse_justify_x(value: &str) -> Result<JustifyX, ParserError> {
    match value.to_uppercase().as_str() {
        "LEFT" => Ok(JustifyX::Left),
        "RIGHT" => Ok(JustifyX::Right),
        "CENTER" => Ok(JustifyX::Center),
        _ => Err(ParserError::ParseError(format!(
            "Invalid JustifyX value: {}",
            value
        ))),
    }
}

fn parse_justify_y(value: &str) -> Result<JustifyY, ParserError> {
    match value.to_uppercase().as_str() {
        "TOP" => Ok(JustifyY::Top),
        "BOTTOM" => Ok(JustifyY::Bottom),
        "CENTER" => Ok(JustifyY::Center),
        _ => Err(ParserError::ParseError(format!(
            "Invalid JustifyY value: {}",
            value
        ))),
    }
}

// Continuing from the previous code with additional parse functions

fn parse_fill_color_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FILL COLOR command requires at least a default value".to_string(),
        ));
    }

    let default = record.get(1).unwrap().to_string();
    let pin_type = record.get(2).map(|s| s.to_string());
    let group = record.get(3).map(|s| s.to_string());

    let mut colors = Vec::new();
    for i in 4..record.len() {
        if let Some(color) = record.get(i) {
            colors.push(color.to_string());
        }
    }

    Ok(Command::FillColor {
        default,
        pin_type,
        group,
        colors,
    })
}

fn parse_opacity_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "OPACITY command requires at least a default value".to_string(),
        ));
    }

    let default = parse_f32(record.get(1).unwrap())?;
    let pin_type = record.get(2).map(|s| parse_f32(s).ok()).flatten();
    let group = record.get(3).map(|s| parse_f32(s).ok()).flatten();

    let mut opacities = Vec::new();
    for i in 4..record.len() {
        if let Some(opacity_str) = record.get(i) {
            if !opacity_str.is_empty() {
                let opacity = parse_f32(opacity_str)?;
                opacities.push(opacity);
            }
        }
    }

    Ok(Command::Opacity {
        default,
        pin_type,
        group,
        opacities,
    })
}

fn parse_font_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT command requires at least a default value".to_string(),
        ));
    }

    let default = record.get(1).unwrap().to_string();
    let pin_type = record.get(2).map(|s| s.to_string());
    let group = record.get(3).map(|s| s.to_string());

    let mut fonts = Vec::new();
    for i in 4..record.len() {
        if let Some(font) = record.get(i) {
            fonts.push(font.to_string());
        }
    }

    Ok(Command::Font {
        default,
        pin_type,
        group,
        fonts,
    })
}

fn parse_font_size_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT SIZE command requires at least a default value".to_string(),
        ));
    }

    let default = parse_f32(record.get(1).unwrap())?;
    let pin_type = record.get(2).map(|s| parse_f32(s).ok()).flatten();
    let group = record.get(3).map(|s| parse_f32(s).ok()).flatten();

    let mut sizes = Vec::new();
    for i in 4..record.len() {
        if let Some(size_str) = record.get(i) {
            if !size_str.is_empty() {
                let size = parse_f32(size_str)?;
                sizes.push(size);
            }
        }
    }

    Ok(Command::FontSize {
        default,
        pin_type,
        group,
        sizes,
    })
}

fn parse_font_color_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT COLOR command requires at least a default value".to_string(),
        ));
    }

    let default = record.get(1).unwrap().to_string();
    let pin_type = record.get(2).map(|s| s.to_string());
    let group = record.get(3).map(|s| s.to_string());

    let mut colors = Vec::new();
    for i in 4..record.len() {
        if let Some(color) = record.get(i) {
            colors.push(color.to_string());
        }
    }

    Ok(Command::FontColor {
        default,
        pin_type,
        group,
        colors,
    })
}

fn parse_font_slant_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT SLANT command requires at least a default value".to_string(),
        ));
    }

    let default = parse_font_slant(record.get(1).unwrap().trim())?;
    let pin_type = record
        .get(2)
        .map(|s| parse_font_slant(s.trim()).ok())
        .flatten();
    let group = record
        .get(3)
        .map(|s| parse_font_slant(s.trim()).ok())
        .flatten();

    let mut slants = Vec::new();
    for i in 4..record.len() {
        if let Some(slant_str) = record.get(i) {
            if !slant_str.trim().is_empty() {
                let slant = parse_font_slant(slant_str.trim())?;
                slants.push(slant);
            }
        }
    }

    Ok(Command::FontSlant {
        default,
        pin_type,
        group,
        slants,
    })
}

fn parse_font_bold_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT BOLD command requires at least a default value".to_string(),
        ));
    }

    let default = parse_font_boldness(record.get(1).unwrap().trim())?;
    let pin_type = record
        .get(2)
        .map(|s| parse_font_boldness(s.trim()).ok())
        .flatten();
    let group = record
        .get(3)
        .map(|s| parse_font_boldness(s.trim()).ok())
        .flatten();

    let mut boldness = Vec::new();
    for i in 4..record.len() {
        if let Some(bold_str) = record.get(i) {
            if !bold_str.trim().is_empty() {
                let bold = parse_font_boldness(bold_str.trim())?;
                boldness.push(bold);
            }
        }
    }

    Ok(Command::FontBold {
        default,
        pin_type,
        group,
        boldness,
    })
}

fn parse_font_stretch_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "FONT STRETCH command requires at least a default value".to_string(),
        ));
    }

    let default = parse_font_stretch(record.get(1).unwrap().trim())?;
    let pin_type = record
        .get(2)
        .map(|s| parse_font_stretch(s.trim()).ok())
        .flatten();
    let group = record
        .get(3)
        .map(|s| parse_font_stretch(s.trim()).ok())
        .flatten();

    let mut stretches = Vec::new();
    for i in 4..record.len() {
        if let Some(stretch_str) = record.get(i) {
            if !stretch_str.is_empty() {
                let stretch = parse_font_stretch(stretch_str.trim())?;
                stretches.push(stretch);
            }
        }
    }

    Ok(Command::FontStretch {
        default,
        pin_type,
        group,
        stretches,
    })
}

fn parse_type_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 4 {
        return Err(ParserError::ParseError(
            "TYPE command requires pin type, color, and opacity".to_string(),
        ));
    }

    let pin_type_str = record.get(1).unwrap().trim().to_uppercase();
    let pin_type = match pin_type_str.as_str() {
        "IO" => PinType::IO,
        "INPUT" => PinType::Input,
        "OUTPUT" => PinType::Output,
        _ => {
            return Err(ParserError::ParseError(format!(
                "Invalid pin type: {}",
                pin_type_str
            )));
        }
    };

    let color = record.get(2).unwrap().to_string();
    let opacity = parse_f32(record.get(3).unwrap())?;

    Ok(Command::Type {
        pin_type,
        color,
        opacity,
    })
}

fn parse_wire_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 5 {
        return Err(ParserError::ParseError(
            "WIRE command requires wire type, color, opacity, and thickness".to_string(),
        ));
    }

    let wire_type_str = record.get(1).unwrap().trim().to_uppercase();
    let wire_type = match wire_type_str.as_str() {
        "DIGITAL" => WireType::Digital,
        "PWM" => WireType::Pwm,
        "ANALOG" => WireType::Analog,
        "HS-ANALOG" => WireType::HsAnalog,
        "POWER" => WireType::Power,
        _ => {
            return Err(ParserError::ParseError(format!(
                "Invalid wire type: {}",
                wire_type_str
            )));
        }
    };

    let color = record.get(2).unwrap().to_string();
    let opacity = parse_f32(record.get(3).unwrap())?;
    let thickness = parse_f32(record.get(4).unwrap())?;

    Ok(Command::Wire {
        wire_type,
        color,
        opacity,
        thickness,
    })
}

fn parse_group_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 4 {
        return Err(ParserError::ParseError(
            "GROUP command requires name, color, and opacity".to_string(),
        ));
    }

    let name = record.get(1).unwrap().to_string();
    let color = record.get(2).unwrap().to_string();
    let opacity = parse_f32(record.get(3).unwrap())?;

    Ok(Command::Group {
        name,
        color,
        opacity,
    })
}

fn parse_box_theme_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 13 {
        return Err(ParserError::ParseError(
            "BOX theme command requires all parameters".to_string(),
        ));
    }

    let name = record.get(1).unwrap().to_string();
    let border_color = record.get(2).unwrap().to_string();
    let border_opacity = parse_f32(record.get(3).unwrap())?;
    let fill_color = record.get(4).unwrap().to_string();
    let fill_opacity = parse_f32(record.get(5).unwrap())?;
    let line_width = parse_f32(record.get(6).unwrap())?;
    let box_width = parse_f32(record.get(7).unwrap())?;
    let box_height = parse_f32(record.get(8).unwrap())?;
    let box_cr_x = parse_f32(record.get(9).unwrap())?;
    let box_cr_y = parse_f32(record.get(10).unwrap())?;
    let box_skew = parse_f32(record.get(11).unwrap())?;
    let box_skew_offset = parse_f32(record.get(12).unwrap())?;

    Ok(Command::BoxTheme {
        name,
        border_color,
        border_opacity,
        fill_color,
        fill_opacity,
        line_width,
        box_width,
        box_height,
        box_cr_x,
        box_cr_y,
        box_skew,
        box_skew_offset,
    })
}

fn parse_text_font_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 9 {
        return Err(ParserError::ParseError(
            "TEXT FONT command requires all parameters".to_string(),
        ));
    }

    let theme_name = record.get(1).unwrap().to_string();
    let font = record.get(2).unwrap().to_string();
    let size = parse_f32(record.get(3).unwrap())?;
    let outline_color = record.get(4).unwrap().to_string();
    let color = record.get(5).unwrap().to_string();
    let slant = parse_font_slant(record.get(6).unwrap().trim())?;
    let bold = parse_font_boldness(record.get(7).unwrap().trim())?;
    let stretch = parse_font_stretch(record.get(8).unwrap().trim())?;

    Ok(Command::TextFont {
        theme_name,
        font,
        size,
        outline_color,
        color,
        slant,
        bold,
        stretch,
    })
}

fn parse_page_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "PAGE command requires a page name".to_string(),
        ));
    }

    let page_name = record.get(1).unwrap().to_string();

    Ok(Command::Page { page_name })
}

fn parse_dpi_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "DPI command requires a DPI value".to_string(),
        ));
    }

    let dpi = parse_u32(record.get(1).unwrap().trim())?;

    Ok(Command::Dpi { dpi })
}

fn parse_google_font_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 2 {
        return Err(ParserError::ParseError(
            "GOOGLEFONT command requires a link".to_string(),
        ));
    }

    let link = record.get(1).unwrap().to_string();

    Ok(Command::GoogleFont { link })
}

fn parse_image_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 6 {
        return Err(ParserError::ParseError(
            "IMAGE command requires name, x, y, w, h parameters".to_string(),
        ));
    }

    let name = record.get(1).unwrap().to_string();

    // Parse x and y as size values that can be either integers or percentages
    // Parse width and height as optional size values
    let x = record
        .get(2)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;
    // Parse width and height as optional size values
    let y = record
        .get(3)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;

    // Parse width and height as optional size values
    let w = record
        .get(4)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;
    let h = record
        .get(5)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;

    // Parse the optional crop parameters
    let cx = record
        .get(6)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;
    let cy = record
        .get(7)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;
    let cw = record
        .get(8)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;
    let ch = record
        .get(9)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;

    // Parse rotation
    let rot = record
        .get(10)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_f32(s))
        .transpose()?;

    Ok(Command::Image {
        name,
        x,
        y,
        w,
        h,
        cx,
        cy,
        cw,
        ch,
        rot,
    })
}

fn parse_icon_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 6 {
        return Err(ParserError::ParseError(
            "ICON command requires name, x, y, w, h parameters".to_string(),
        ));
    }

    let name = record.get(1).unwrap().to_string();

    // Parse x and y as size values that can be either integers or percentages
    let x = record
        .get(2)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;

    let y = record
        .get(3)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;

    // Parse width and height as size values
    let w = record
        .get(4)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;

    let h = record
        .get(5)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_size(s))
        .transpose()?;

    // Parse rotation as an optional parameter
    let rot = record
        .get(6)
        .filter(|s| !s.trim().is_empty())
        .map(|s| parse_f32(s))
        .transpose()?;

    Ok(Command::Icon {
        name,
        x,
        y,
        w,
        h,
        rot,
    })
}

fn parse_anchor_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 3 {
        return Err(ParserError::ParseError(
            "ANCHOR command requires x and y parameters".to_string(),
        ));
    }

    let x = parse_f32(record.get(1).unwrap())?;
    let y = parse_f32(record.get(2).unwrap())?;

    Ok(Command::Anchor { x, y })
}

fn parse_pinset_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 11 {
        return Err(ParserError::ParseError(
            "PINSET command requires all parameters".to_string(),
        ));
    }

    let side_str = record.get(1).unwrap().trim().to_uppercase();
    let side = match side_str.as_str() {
        "LEFT" => Side::Left,
        "RIGHT" => Side::Right,
        "TOP" => Side::Top,
        "BOTTOM" => Side::Bottom,
        _ => {
            return Err(ParserError::ParseError(format!(
                "Invalid side: {}",
                side_str
            )));
        }
    };

    let packed_str = record.get(2).unwrap().trim().to_uppercase();
    let packed = match packed_str.as_str() {
        "TRUE" | "YES" | "1" | "PACKED" => true,
        "FALSE" | "NO" | "0" | "UNPACKED" => false,
        _ => {
            return Err(ParserError::ParseError(format!(
                "Invalid packed value: {}",
                packed_str
            )));
        }
    };

    let justify_x = parse_justify_x(record.get(3).unwrap().trim())?;
    let justify_y = parse_justify_y(record.get(4).unwrap().trim())?;
    let line_step = parse_f32(record.get(5).unwrap())?;
    let pin_width = parse_f32(record.get(6).unwrap())?;
    let group_width = parse_f32(record.get(7).unwrap())?;
    let leader_offset = parse_f32(record.get(8).unwrap())?;
    let column_gap = parse_f32(record.get(9).unwrap())?;
    let leader_h_step = parse_f32(record.get(10).unwrap())?;

    Ok(Command::PinSet {
        side,
        packed,
        justify_x,
        justify_y,
        line_step,
        pin_width,
        group_width,
        leader_offset,
        column_gap,
        leader_h_step,
    })
}

fn parse_pin_text_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 6 {
        return Err(ParserError::ParseError(
            "PINTEXT command requires theme and text parameters".to_string(),
        ));
    }

    let wire = record.get(1).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            match s.to_uppercase().as_str() {
                "DIGITAL" => Some(WireType::Digital),
                "PWM" => Some(WireType::Pwm),
                "ANALOG" => Some(WireType::Analog),
                "HS-ANALOG" => Some(WireType::HsAnalog),
                "POWER" => Some(WireType::Power),
                _ => None,
            }
        }
    });

    let pin_type = record.get(2).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            match s.to_uppercase().as_str() {
                "IO" => Some(PinType::IO),
                "INPUT" => Some(PinType::Input),
                "OUTPUT" => Some(PinType::Output),
                _ => None,
            }
        }
    });

    let group = record.get(3).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    let theme = record.get(4).unwrap().to_string();

    let label = record.get(5).and_then(|s| {
        if s.is_empty() {
            None
        } else {
            Some(s.to_string())
        }
    });

    let text = record.get(6).unwrap_or("").to_string();

    Ok(Command::PinText {
        wire,
        pin_type,
        group,
        theme,
        label,
        text,
    })
}

fn parse_box_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 4 {
        return Err(ParserError::ParseError(
            "BOX command requires theme, x, y parameters".to_string(),
        ));
    }

    let theme = record.get(1).unwrap().to_string();
    let x = parse_f32(record.get(2).unwrap())?;
    let y = parse_f32(record.get(3).unwrap())?;

    let box_width = record.get(4).and_then(|s| parse_f32(s).ok());
    let box_height = record.get(5).and_then(|s| parse_f32(s).ok());
    let x_justify = record.get(6).and_then(|s| parse_justify_x(s.trim()).ok());
    let y_justify = record.get(7).and_then(|s| parse_justify_y(s.trim()).ok());
    let text = record.get(8).map(|s| s.to_string());

    Ok(Command::Box {
        theme,
        x,
        y,
        box_width,
        box_height,
        x_justify,
        y_justify,
        text,
    })
}

fn parse_message_command(record: &StringRecord) -> Result<Command, ParserError> {
    let x = record.get(1).and_then(|s| parse_f32(s).ok());
    let y = record.get(2).and_then(|s| parse_f32(s).ok());
    let line_step = record.get(3).and_then(|s| parse_f32(s).ok());
    let font = record.get(4).map(|s| s.to_string());
    let font_size = record.get(5).and_then(|s| parse_f32(s).ok());
    let x_justify = record.get(6).and_then(|s| parse_justify_x(s.trim()).ok());
    let y_justify = record.get(7).and_then(|s| parse_justify_y(s.trim()).ok());

    Ok(Command::Message {
        x,
        y,
        line_step,
        font,
        font_size,
        x_justify,
        y_justify,
    })
}

fn parse_text_command(record: &StringRecord) -> Result<Command, ParserError> {
    if record.len() < 4 {
        return Err(ParserError::ParseError(
            "TEXT command requires edge color, color, and message parameters".to_string(),
        ));
    }

    let edge_color = record.get(1).unwrap().to_string();
    let color = record.get(2).unwrap().to_string();
    let message = record.get(3).unwrap().to_string();

    let new_line = record.get(4).is_some();

    Ok(Command::Text {
        edge_color,
        color,
        message,
        new_line,
    })
}

// Helper functions for parsing specific types
fn parse_font_slant(value: &str) -> Result<FontSlant, ParserError> {
    match value.to_lowercase().as_str() {
        "normal" => Ok(FontSlant::Normal),
        "italic" => Ok(FontSlant::Italic),
        "oblique" => Ok(FontSlant::Oblique),
        _ => Err(ParserError::ParseError(format!(
            "Invalid font slant: {}",
            value
        ))),
    }
}

fn parse_font_boldness(value: &str) -> Result<FontBoldness, ParserError> {
    match value.to_lowercase().as_str() {
        "normal" => Ok(FontBoldness::Normal),
        "bold" => Ok(FontBoldness::Bold),
        "bolder" => Ok(FontBoldness::Bolder),
        "lighter" => Ok(FontBoldness::Lighter),
        "100" => Ok(FontBoldness::Weight100),
        "200" => Ok(FontBoldness::Weight200),
        "300" => Ok(FontBoldness::Weight300),
        "400" => Ok(FontBoldness::Weight400),
        "500" => Ok(FontBoldness::Weight500),
        "600" => Ok(FontBoldness::Weight600),
        "700" => Ok(FontBoldness::Weight700),
        "800" => Ok(FontBoldness::Weight800),
        "900" => Ok(FontBoldness::Weight900),
        _ => Err(ParserError::ParseError(format!(
            "Invalid font boldness: {}",
            value
        ))),
    }
}

fn parse_font_stretch(value: &str) -> Result<FontStretch, ParserError> {
    match value.to_lowercase().as_str() {
        "normal" => Ok(FontStretch::Normal),
        "wider" => Ok(FontStretch::Wider),
        "narrower" => Ok(FontStretch::Narrower),
        "ultra-condensed" => Ok(FontStretch::UltraCondensed),
        "extra-condensed" => Ok(FontStretch::ExtraCondensed),
        "condensed" => Ok(FontStretch::Condensed),
        "semi-condensed" => Ok(FontStretch::SemiCondensed),
        "semi-expanded" => Ok(FontStretch::SemiExpanded),
        "expanded" => Ok(FontStretch::Expanded),
        "extra-expanded" => Ok(FontStretch::ExtraExpanded),
        "ultra-expanded" => Ok(FontStretch::UltraExpanded),
        _ => Err(ParserError::ParseError(format!(
            "Invalid font stretch: {}",
            value
        ))),
    }
}

fn parse_side(value: &str) -> Result<Side, ParserError> {
    match value.to_uppercase().as_str() {
        "LEFT" => Ok(Side::Left),
        "RIGHT" => Ok(Side::Right),
        "TOP" => Ok(Side::Top),
        "BOTTOM" => Ok(Side::Bottom),
        _ => Err(ParserError::ParseError(format!("Invalid side: {}", value))),
    }
}

fn parse_size(value: &str) -> Result<f32, ParserError> {
    if value.is_empty() {
        return Err(ParserError::ParseError("Empty size value".to_string()));
    }

    // Check if it's a percentage
    if value.ends_with('%') {
        if let Some(percent_str) = value.trim().strip_suffix('%') {
            if let Ok(percent_val) = percent_str.parse::<f32>() {
                // Convert percentage to a value between 0.0 and 0.9999
                // where 0.9999 represents 100%
                let normalized = (0.9999 * f32::min(percent_val, 100.0)) / 100.0;
                return Ok(normalized);
            }
        }
        return Err(ParserError::ParseError(format!(
            "Failed to parse percentage: {}",
            value
        )));
    } else {
        // Try to parse as a regular number
        parse_f32(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv::StringRecord;

    #[test]
    fn test_parse_image_command() {
        // Create a StringRecord that simulates the CSV input line
        let record = StringRecord::from(vec![
            "IMAGE",
            "Resources/TopView-FLAT-Transparent-R110.png",
            " 1750",
            " 1500",
            " ",
            " ",
            " ",
            " ",
            " ",
            " ",
            " -90",
        ]);

        // Parse the record
        let result = parse_image_command(&record);

        // Verify the result is Ok
        assert!(
            result.is_ok(),
            "Failed to parse image command: {:?}",
            result.err()
        );

        // Extract the command and verify its properties
        if let Ok(Command::Image {
            name,
            x,
            y,
            w,
            h,
            cx,
            cy,
            cw,
            ch,
            rot,
        }) = result
        {
            // Check the name
            assert_eq!(name, "Resources/TopView-FLAT-Transparent-R110.png");

            // Check x and y values
            assert_eq!(x.unwrap(), 1750.0);
            assert_eq!(y.unwrap(), 1500.0);

            // Check that optional values are correctly parsed as None
            assert!(w.is_none(), "Width should be None but was {:?}", w);
            assert!(h.is_none(), "Height should be None but was {:?}", h);
            assert!(cx.is_none(), "Crop x should be None but was {:?}", cx);
            assert!(cy.is_none(), "Crop y should be None but was {:?}", cy);
            assert!(cw.is_none(), "Crop width should be None but was {:?}", cw);
            assert!(ch.is_none(), "Crop height should be None but was {:?}", ch);

            // Check rotation value
            assert_eq!(rot.unwrap(), -90.0);
        } else {
            panic!("Expected Command::Image, got something else: {:?}", result);
        }
    }

    #[test]
    fn test_parse_image_command_with_percentages() {
        // Create a StringRecord with percentage values
        let record = StringRecord::from(vec![
            "IMAGE",
            "Resources/test.png",
            "50%",
            "75%",
            "25%",
            "30%",
            "",
            "",
            "",
            "",
            "",
        ]);

        // Parse the record
        let result = parse_image_command(&record);

        // Verify the result is Ok
        assert!(
            result.is_ok(),
            "Failed to parse image command: {:?}",
            result.err()
        );

        // Extract the command and verify percentage values are converted correctly
        if let Ok(Command::Image {
            name,
            x,
            y,
            w,
            h,
            cx,
            cy,
            cw,
            ch,
            rot,
        }) = result
        {
            // Check the name
            assert_eq!(name, "Resources/test.png");

            // Check percentage values (converted to 0.0-0.9999 range)
            assert!(
                (x.unwrap() - 0.49995).abs() < 0.0001,
                "Expected x to be ~0.49995 but got {:?}",
                x
            );

            assert!(
                (y.unwrap() - 0.749925).abs() < 0.0001,
                "Expected y to be ~0.749925 but got {:?}",
                y
            );

            assert!(
                (w.unwrap() - 0.249975).abs() < 0.0001,
                "Expected w to be ~0.249975 but got {:?}",
                w
            );

            assert!(
                (h.unwrap() - 0.29997).abs() < 0.0001,
                "Expected h to be ~0.29997 but got {:?}",
                h
            );

            // Check that other optional values are None
            assert!(cx.is_none());
            assert!(cy.is_none());
            assert!(cw.is_none());
            assert!(ch.is_none());
            assert!(rot.is_none());
        } else {
            panic!("Expected Command::Image, got something else: {:?}", result);
        }
    }
}
