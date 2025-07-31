use crate::parser::types::{
    Command, FontBoldness, FontSlant, FontStretch, JustifyX, JustifyY, Phase, PinType, Side,
    WireType,
};
use base64::{Engine, encode, engine::general_purpose};
use image::{GenericImageView, ImageFormat};
use std::collections::HashMap;
use std::error::Error;
use std::f32::consts::PI;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use svg::Document;
use svg::node::element::{
    Circle, Definitions, Group, Image, Line, Path as SvgPath, Polygon, Polyline, Rectangle, Style,
    TSpan, Text,
};
use svg::node::{Text as TextNode, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("SVG rendering error: {0}")]
    SvgError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Image error: {0}")]
    ImageError(#[from] image::ImageError),

    #[error("Invalid phase: expected {expected:?}, got {got:?}")]
    InvalidPhase { expected: Phase, got: Phase },

    #[error("Missing required command data: {0}")]
    MissingData(String),
}

pub struct SvgRenderer {
    document: Document,
    page_dimensions: (f32, f32), // mm
    page_resolution: (u32, u32), // pixels
    dpi: u32,
    page_type: String,
    themes: HashMap<String, HashMap<String, String>>,
    anchor_x: f32,
    anchor_y: f32,
    offset_x: f32,
    offset_y: f32,
    line_settings: HashMap<String, Value>,
    message_settings: HashMap<String, Value>,
    current_text: Option<Text>,
    pin_func_types: Vec<String>,
    definitions: Definitions,
}

impl SvgRenderer {
    pub fn new() -> Self {
        let page_type = "A4-L".to_string();
        let dpi = 300;

        // Default page dimensions for A4 landscape
        let page_dimensions = (297.0, 210.0); // mm

        // Calculate resolution in pixels based on DPI
        let page_resolution = (
            ((page_dimensions.0 * dpi as f32) / 25.4) as u32,
            ((page_dimensions.1 * dpi as f32) / 25.4) as u32,
        );

        // Create the SVG document with the calculated dimensions
        let document = Document::new()
            .set("viewBox", (0, 0, page_resolution.0, page_resolution.1))
            .set("width", format!("{}mm", page_dimensions.0))
            .set("height", format!("{}mm", page_dimensions.1));

        SvgRenderer {
            document,
            page_dimensions,
            page_resolution,
            dpi,
            page_type,
            themes: HashMap::new(),
            anchor_x: 0.0,
            anchor_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            line_settings: HashMap::new(),
            message_settings: HashMap::new(),
            current_text: None,
            pin_func_types: Vec::new(),
            definitions: Definitions::new(),
        }
    }

    pub fn process_commands(&mut self, commands: &[Command]) -> Result<(), RenderError> {
        let mut phase = Phase::Setup;

        for command in commands {
            match (command, phase) {
                (Command::Draw, Phase::Setup) => {
                    // Transition from Setup to Draw phase
                    self.check_boxes()?;
                    phase = Phase::Draw;
                }
                (cmd, current_phase) => {
                    let cmd_phase = self.get_command_phase(cmd);
                    if cmd_phase != current_phase {
                        return Err(RenderError::InvalidPhase {
                            expected: cmd_phase,
                            got: current_phase,
                        });
                    }

                    self.execute_command(cmd)?;
                }
            }
        }

        // Ensure any open text message is closed
        if self.current_text.is_some() {
            self.end_message()?;
        }

        // Add definitions to document
        self.document = self.document.clone().add(self.definitions.clone());

        Ok(())
    }

    fn get_command_phase(&self, command: &Command) -> Phase {
        match command {
            Command::Draw => Phase::Setup, // Special case handled separately

            // Setup phase commands
            Command::Labels { .. } => Phase::Setup,
            Command::BorderColor { .. } => Phase::Setup,
            Command::BorderWidth { .. } => Phase::Setup,
            Command::BorderOpacity { .. } => Phase::Setup,
            Command::FillColor { .. } => Phase::Setup,
            Command::Opacity { .. } => Phase::Setup,
            Command::Font { .. } => Phase::Setup,
            Command::FontSize { .. } => Phase::Setup,
            Command::FontColor { .. } => Phase::Setup,
            Command::FontSlant { .. } => Phase::Setup,
            Command::FontBold { .. } => Phase::Setup,
            Command::FontStretch { .. } => Phase::Setup,
            Command::FontOutline { .. } => Phase::Setup,
            Command::FontOutlineThickness { .. } => Phase::Setup,
            Command::Type { .. } => Phase::Setup,
            Command::Wire { .. } => Phase::Setup,
            Command::Group { .. } => Phase::Setup,
            Command::BoxTheme { .. } => Phase::Setup,
            Command::TextFont { .. } => Phase::Setup,
            Command::Page { .. } => Phase::Setup,
            Command::Dpi { .. } => Phase::Setup,

            // Draw phase commands
            Command::GoogleFont { .. } => Phase::Draw,
            Command::Image { .. } => Phase::Draw,
            Command::Icon { .. } => Phase::Draw,
            Command::Anchor { .. } => Phase::Draw,
            Command::PinSet { .. } => Phase::Draw,
            Command::Pin { .. } => Phase::Draw,
            Command::PinText { .. } => Phase::Draw,
            Command::Box { .. } => Phase::Draw,
            Command::Message { .. } => Phase::Draw,
            Command::Text { .. } => Phase::Draw,
            Command::EndMessage => Phase::Draw,
        }
    }

    fn execute_command(&mut self, command: &Command) -> Result<(), RenderError> {
        match command {
            // Setup phase commands
            Command::Labels {
                default,
                pin_type,
                group,
                labels,
            } => self.set_labels(default, pin_type, group, labels),
            Command::BorderColor {
                default,
                pin_type,
                group,
                colors,
            } => self.set_theme_str("Border Color", default, pin_type, group, colors),
            Command::BorderWidth { width } => self.set_border_width(width),
            Command::BorderOpacity { opacity } => self.set_border_opacity(opacity),
            Command::FillColor {
                default,
                pin_type,
                group,
                colors,
            } => self.set_theme_str("Fill Color", default, pin_type, group, colors),
            Command::Opacity {
                default,
                pin_type,
                group,
                opacities,
            } => self.set_theme_float("Opacity", *default, *pin_type, *group, opacities),
            Command::Font {
                default,
                pin_type,
                group,
                fonts,
            } => self.set_theme_str("Font", default, pin_type, group, fonts),
            Command::FontSize {
                default,
                pin_type,
                group,
                sizes,
            } => self.set_theme_float("Font Size", *default, *pin_type, *group, sizes),
            Command::FontColor {
                default,
                pin_type,
                group,
                colors,
            } => self.set_theme_str("Font Color", default, pin_type, group, colors),
            Command::FontSlant {
                default,
                pin_type,
                group,
                slants,
            } => self.set_font_slant(*default, *pin_type, *group, slants),
            Command::FontBold {
                default,
                pin_type,
                group,
                boldness,
            } => self.set_font_bold(*default, *pin_type, *group, boldness),
            Command::FontStretch {
                default,
                pin_type,
                group,
                stretches,
            } => self.set_font_stretch(*default, *pin_type, *group, stretches),
            Command::FontOutline {
                default,
                pin_type,
                group,
                colors,
            } => self.set_theme_str("Font Outline", default, pin_type, group, colors),
            Command::FontOutlineThickness {
                default,
                pin_type,
                group,
                thickness,
            } => self.set_theme_float(
                "Font Outline Thickness",
                *default,
                *pin_type,
                *group,
                thickness,
            ),
            Command::Type {
                pin_type,
                color,
                opacity,
            } => self.set_pin_type(*pin_type, color, *opacity),
            Command::Wire {
                wire_type,
                color,
                opacity,
                thickness,
            } => self.set_wire_type(*wire_type, color, *opacity, *thickness),
            Command::Group {
                name,
                color,
                opacity,
            } => self.set_group(name, color, *opacity),
            Command::BoxTheme {
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
            } => self.define_box(
                name,
                border_color,
                *border_opacity,
                fill_color,
                *fill_opacity,
                *line_width,
                *box_width,
                *box_height,
                *box_cr_x,
                *box_cr_y,
                *box_skew,
                *box_skew_offset,
            ),
            Command::TextFont {
                theme_name,
                font,
                size,
                outline_color,
                color,
                slant,
                bold,
                stretch,
            } => self.define_text_font(
                theme_name,
                font,
                *size,
                outline_color,
                color,
                *slant,
                *bold,
                *stretch,
            ),
            Command::Page { page_name } => self.set_page_size(page_name),
            Command::Dpi { dpi } => self.set_dpi(*dpi),

            // Draw phase commands
            Command::Draw => Ok(()), // Already handled in process_commands
            Command::GoogleFont { link } => todo!("handle font implementation"),
            Command::Image {
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
            } => self.write_image(name, *x, *y, *w, *h, *cx, *cy, *cw, *ch, *rot),
            Command::Icon {
                name,
                x,
                y,
                w,
                h,
                rot,
            } => self.write_icon(name, *x, *y, *w, *h, *rot),
            Command::Anchor { x, y } => self.move_anchor(*x, *y),
            Command::PinSet {
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
            } => self.start_pin_set(
                *side,
                *packed,
                *justify_x,
                *justify_y,
                *line_step,
                *pin_width,
                *group_width,
                *leader_offset,
                *column_gap,
                *leader_h_step,
            ),
            Command::Pin {
                wire,
                pin_type,
                group,
                attributes,
            } => self.write_pin(*wire, *pin_type, group, attributes),
            Command::PinText {
                wire,
                pin_type,
                group,
                theme,
                label,
                text,
            } => self.write_pin_text(*wire, *pin_type, group, theme, label, text),
            Command::Box {
                theme,
                x,
                y,
                box_width,
                box_height,
                x_justify,
                y_justify,
                text,
            } => self.draw_box(
                theme,
                *x,
                *y,
                *box_width,
                *box_height,
                *x_justify,
                *y_justify,
                text,
            ),
            Command::Message {
                x,
                y,
                line_step,
                font,
                font_size,
                x_justify,
                y_justify,
            } => self
                .start_text_message(*x, *y, *line_step, font, *font_size, *x_justify, *y_justify),
            Command::Text {
                edge_color,
                color,
                message,
                new_line,
            } => self.write_text(edge_color, color, message, *new_line),
            Command::EndMessage => self.end_message(),
        }
    }

    fn set_labels(
        &mut self,
        default: &str,
        pin_type: &Option<String>,
        group: &Option<String>,
        labels: &[String],
    ) -> Result<(), RenderError> {
        // Define fixed theme entries
        let fixed_theme_entries = vec![
            "DEFAULT".to_string(),
            "TYPE".to_string(),
            "GROUP".to_string(),
        ];

        // Check if pin function types have already been initialized
        if self.pin_func_types.is_empty() {
            // Verify that default matches the first fixed entry
            if default == "DEFAULT"
                && (pin_type.is_none() || pin_type.as_ref().unwrap() == "TYPE")
                && (group.is_none() || group.as_ref().unwrap() == "GROUP")
            {
                // Set pin_func_types to just the labels
                self.pin_func_types = labels.to_vec();

                // Initialize empty theme dictionaries for fixed entries and labels
                for entry in &fixed_theme_entries {
                    self.themes.insert(entry.clone(), HashMap::new());
                }

                for label in labels {
                    self.themes.insert(label.clone(), HashMap::new());
                }

                Ok(())
            } else {
                Err(RenderError::SvgError(format!(
                    "Error: First labels must be {:?}!",
                    fixed_theme_entries
                )))
            }
        } else {
            Err(RenderError::SvgError(
                "Error: Can only set the pin function labels ONCE!".to_string(),
            ))
        }
    }

    fn set_theme_str(
        &mut self,
        entry: &str,
        default: &str,
        pin_type: &Option<String>,
        group: &Option<String>,
        values: &[String],
    ) -> Result<(), RenderError> {
        // Set the theme entry for the default theme
        self.set_theme_value("DEFAULT", entry, default.to_string());

        // Set for pin type if provided
        if let Some(pt) = pin_type {
            self.set_theme_value("TYPE", entry, pt.clone());
        }

        // Set for group if provided
        if let Some(g) = group {
            self.set_theme_value("GROUP", entry, g.clone());
        }

        // Set for each pin function type
        for (i, value) in values.iter().enumerate() {
            if i < self.pin_func_types.len() {
                let pin_func = &self.pin_func_types[i].clone();
                self.set_theme_value(pin_func, entry, value.clone());
            }
        }

        Ok(())
    }

    fn set_theme_float(
        &mut self,
        entry: &str,
        default: f32,
        pin_type: Option<f32>,
        group: Option<f32>,
        values: &[f32],
    ) -> Result<(), RenderError> {
        // Set the theme entry for the default theme
        self.set_theme_value("DEFAULT", entry, default.to_string());

        // Set for pin type if provided
        if let Some(pt) = pin_type {
            self.set_theme_value("TYPE", entry, pt.to_string());
        }

        // Set for group if provided
        if let Some(g) = group {
            self.set_theme_value("GROUP", entry, g.to_string());
        }

        // Set for each pin function type
        for (i, &value) in values.iter().enumerate() {
            if i < self.pin_func_types.len() {
                let pin_func = &self.pin_func_types[i].clone();
                self.set_theme_value(pin_func, entry, value.to_string());
            }
        }

        Ok(())
    }

    fn set_theme_value(&mut self, theme: &str, entry: &str, value: String) {
        if let Some(theme_map) = self.themes.get_mut(theme) {
            theme_map.insert(entry.to_string(), value);
        } else {
            let mut theme_map = HashMap::new();
            theme_map.insert(entry.to_string(), value);
            self.themes.insert(theme.to_string(), theme_map);
        }
    }

    fn set_border_width(&mut self, width: &str) -> Result<(), RenderError> {
        self.set_theme_value("DEFAULT", "Border Width", width.to_string());
        Ok(())
    }

    fn set_border_opacity(&mut self, opacity: &str) -> Result<(), RenderError> {
        self.set_theme_value("DEFAULT", "Border Opacity", opacity.to_string());
        Ok(())
    }

    fn set_font_slant(
        &mut self,
        default: FontSlant,
        pin_type: Option<FontSlant>,
        group: Option<FontSlant>,
        slants: &[FontSlant],
    ) -> Result<(), RenderError> {
        // Convert FontSlant enum to string
        let default_str = font_slant_to_string(default);

        // Set the theme entry for the default theme
        self.set_theme_value("DEFAULT", "Font Slant", default_str);

        // Set for pin type if provided
        if let Some(pt) = pin_type {
            self.set_theme_value("TYPE", "Font Slant", font_slant_to_string(pt));
        }

        // Set for group if provided
        if let Some(g) = group {
            self.set_theme_value("GROUP", "Font Slant", font_slant_to_string(g));
        }

        // Set for each pin function type
        for (i, &slant) in slants.iter().enumerate() {
            if i < self.pin_func_types.len() {
                let pin_func = &self.pin_func_types[i].clone();
                self.set_theme_value(pin_func, "Font Slant", font_slant_to_string(slant));
            }
        }

        Ok(())
    }

    fn set_font_bold(
        &mut self,
        default: FontBoldness,
        pin_type: Option<FontBoldness>,
        group: Option<FontBoldness>,
        boldness: &[FontBoldness],
    ) -> Result<(), RenderError> {
        // Convert FontBoldness enum to string
        let default_str = font_boldness_to_string(default);

        // Set the theme entry for the default theme
        self.set_theme_value("DEFAULT", "Font Bold", default_str);

        // Set for pin type if provided
        if let Some(pt) = pin_type {
            self.set_theme_value("TYPE", "Font Bold", font_boldness_to_string(pt));
        }

        // Set for group if provided
        if let Some(g) = group {
            self.set_theme_value("GROUP", "Font Bold", font_boldness_to_string(g));
        }

        // Set for each pin function type
        for (i, &bold) in boldness.iter().enumerate() {
            if i < self.pin_func_types.len() {
                let pin_func = &self.pin_func_types[i].clone();
                self.set_theme_value(pin_func, "Font Bold", font_boldness_to_string(bold));
            }
        }

        Ok(())
    }

    fn set_font_stretch(
        &mut self,
        default: FontStretch,
        pin_type: Option<FontStretch>,
        group: Option<FontStretch>,
        stretches: &[FontStretch],
    ) -> Result<(), RenderError> {
        // Convert FontStretch enum to string
        let default_str = font_stretch_to_string(default);

        // Set the theme entry for the default theme
        self.set_theme_value("DEFAULT", "Font Stretch", default_str);

        // Set for pin type if provided
        if let Some(pt) = pin_type {
            self.set_theme_value("TYPE", "Font Stretch", font_stretch_to_string(pt));
        }

        // Set for group if provided
        if let Some(g) = group {
            self.set_theme_value("GROUP", "Font Stretch", font_stretch_to_string(g));
        }

        // Set for each pin function type
        for (i, &stretch) in stretches.iter().enumerate() {
            if i < self.pin_func_types.len() {
                let pin_func = &self.pin_func_types[i].clone();
                self.set_theme_value(pin_func, "Font Stretch", font_stretch_to_string(stretch));
            }
        }

        Ok(())
    }

    fn set_pin_type(
        &mut self,
        pin_type: PinType,
        color: &str,
        opacity: f32,
    ) -> Result<(), RenderError> {
        let theme_entry = format!("PINTYPE_{}", pin_type);

        // Create or get the theme map
        let theme_map = self.themes.entry(theme_entry).or_insert_with(HashMap::new);

        // Set the color and opacity
        theme_map.insert("FILL COLOR".to_string(), color.to_string());
        theme_map.insert("OPACITY".to_string(), opacity.to_string());

        Ok(())
    }

    fn set_wire_type(
        &mut self,
        wire_type: WireType,
        color: &str,
        opacity: f32,
        thickness: f32,
    ) -> Result<(), RenderError> {
        let theme_entry = format!("PINWIRE_{}", wire_type);

        // Create or get the theme map
        let theme_map = self.themes.entry(theme_entry).or_insert_with(HashMap::new);

        // Set the color, opacity, and thickness
        theme_map.insert("FILL COLOR".to_string(), color.to_string());
        theme_map.insert("OPACITY".to_string(), opacity.to_string());
        theme_map.insert("THICKNESS".to_string(), thickness.to_string());

        Ok(())
    }

    fn set_group(&mut self, name: &str, color: &str, opacity: f32) -> Result<(), RenderError> {
        let theme_entry = format!("GROUP_{}", name);

        // Create or get the theme map
        let theme_map = self.themes.entry(theme_entry).or_insert_with(HashMap::new);

        // Set the color and opacity
        theme_map.insert("FILL COLOR".to_string(), color.to_string());
        theme_map.insert("OPACITY".to_string(), opacity.to_string());

        Ok(())
    }

    fn define_box(
        &mut self,
        name: &str,
        border_color: &str,
        border_opacity: f32,
        fill_color: &str,
        fill_opacity: f32,
        line_width: f32,
        box_width: f32,
        box_height: f32,
        box_cr_x: f32,
        box_cr_y: f32,
        box_skew: f32,
        box_skew_offset: f32,
    ) -> Result<(), RenderError> {
        let theme_entry = format!("BOX_{}", name);

        // Create or get the theme map
        let theme_map = self.themes.entry(theme_entry).or_insert_with(HashMap::new);

        // Set all box theme parameters
        theme_map.insert("BORDER COLOR".to_string(), border_color.to_string());
        theme_map.insert("BORDER OPACITY".to_string(), border_opacity.to_string());
        theme_map.insert("FILL COLOR".to_string(), fill_color.to_string());
        theme_map.insert("OPACITY".to_string(), fill_opacity.to_string());
        theme_map.insert("BORDER WIDTH".to_string(), line_width.to_string());
        theme_map.insert("WIDTH".to_string(), box_width.to_string());
        theme_map.insert("HEIGHT".to_string(), box_height.to_string());
        theme_map.insert("CORNER RX".to_string(), box_cr_x.to_string());
        theme_map.insert("CORNER RY".to_string(), box_cr_y.to_string());
        theme_map.insert("SKEW".to_string(), box_skew.to_string());
        theme_map.insert("SKEW OFFSET".to_string(), box_skew_offset.to_string());

        Ok(())
    }

    fn define_text_font(
        &mut self,
        theme_name: &str,
        font: &str,
        size: f32,
        outline_color: &str,
        color: &str,
        slant: FontSlant,
        bold: FontBoldness,
        stretch: FontStretch,
    ) -> Result<(), RenderError> {
        let theme_entry = format!("FONT_{}", theme_name);

        // Create or get the theme map
        let theme_map = self.themes.entry(theme_entry).or_insert_with(HashMap::new);

        // Set all text font parameters
        theme_map.insert("FONT".to_string(), font.to_string());
        theme_map.insert("FONT SIZE".to_string(), size.to_string());
        theme_map.insert("OUTLINE COLOR".to_string(), outline_color.to_string());
        theme_map.insert("FONT COLOR".to_string(), color.to_string());
        theme_map.insert("FONT SLANT".to_string(), font_slant_to_string(slant));
        theme_map.insert("FONT BOLD".to_string(), font_boldness_to_string(bold));
        theme_map.insert("FONT STRETCH".to_string(), font_stretch_to_string(stretch));

        Ok(())
    }

    fn set_page_size(&mut self, page_name: &str) -> Result<(), RenderError> {
        let page_dimensions = match page_name {
            "A4-P" => (210.0, 297.0), // mm (portrait)
            "A4-L" => (297.0, 210.0), // mm (landscape)
            "A3-P" => (297.0, 420.0), // mm (portrait)
            "A3-L" => (420.0, 297.0), // mm (landscape)
            _ => {
                return Err(RenderError::SvgError(format!(
                    "Unknown page type: {}",
                    page_name
                )));
            }
        };

        self.page_type = page_name.to_string();
        self.page_dimensions = page_dimensions;

        // Recalculate resolution in pixels based on DPI
        self.page_resolution = (
            ((self.page_dimensions.0 * self.dpi as f32) / 25.4) as u32,
            ((self.page_dimensions.1 * self.dpi as f32) / 25.4) as u32,
        );

        // Update the document dimensions
        self.document = self
            .document
            .clone()
            .set(
                "viewBox",
                (0, 0, self.page_resolution.0, self.page_resolution.1),
            )
            .set("width", format!("{}mm", self.page_dimensions.0))
            .set("height", format!("{}mm", self.page_dimensions.1));

        Ok(())
    }

    fn set_dpi(&mut self, dpi: u32) -> Result<(), RenderError> {
        if dpi < 50 || dpi > 1200 {
            return Err(RenderError::SvgError(
                "DPI must be between 50 and 1200".to_string(),
            ));
        }

        self.dpi = dpi;

        // Recalculate resolution in pixels based on new DPI
        self.page_resolution = (
            ((self.page_dimensions.0 * dpi as f32) / 25.4) as u32,
            ((self.page_dimensions.1 * dpi as f32) / 25.4) as u32,
        );

        // Update the document dimensions
        self.document = self
            .document
            .clone()
            .set(
                "viewBox",
                (0, 0, self.page_resolution.0, self.page_resolution.1),
            )
            .set("width", format!("{}mm", self.page_dimensions.0))
            .set("height", format!("{}mm", self.page_dimensions.1));

        Ok(())
    }

    fn check_boxes(&self) -> Result<(), RenderError> {
        for (theme_name, theme_map) in &self.themes {
            if let Some(boxes) = theme_map.get("BOXES") {
                let box_theme = format!("BOX_{}", boxes);
                if !self.themes.contains_key(&box_theme) {
                    return Err(RenderError::SvgError(format!(
                        "Box {} used for {} theme, but not defined!",
                        boxes, theme_name
                    )));
                }
            }
        }
        Ok(())
    }

    fn write_image(
        &mut self,
        name: &str,
        x: Option<f32>,
        y: Option<f32>,
        w: Option<f32>,
        h: Option<f32>,
        cx: Option<f32>,
        cy: Option<f32>,
        cw: Option<f32>,
        ch: Option<f32>,
        rot: Option<f32>,
    ) -> Result<(), RenderError> {
        let path = Path::new(name);
        if !path.exists() {
            return Err(RenderError::SvgError(format!(
                "Image file not found: {}",
                name
            )));
        }

        // Load the image
        let mut img = image::open(path)?;

        // Apply crop if all crop parameters are provided
        let img = if cx.is_some() && cy.is_some() && cw.is_some() && ch.is_some() {
            let cx = cx.unwrap() as u32;
            let cy = cy.unwrap() as u32;
            let cw = cw.unwrap() as u32;
            let ch = ch.unwrap() as u32;

            // Check if crop coordinates are valid
            if cx + cw > img.width() || cy + ch > img.height() {
                return Err(RenderError::SvgError("Invalid crop parameters".to_string()));
            }

            img.crop(cx, cy, cw, ch)
        } else if cx.is_some() || cy.is_some() || cw.is_some() || ch.is_some() {
            return Err(RenderError::SvgError(
                "Crop parameters cx, cy, cw, ch must all be specified, or none".to_string(),
            ));
        } else {
            img
        };

        // Resize if width or height is specified
        let img = if w.is_some() || h.is_some() {
            let w = get_size(w, img.width() as f32, None) as u32;
            let h = get_size(h, img.height() as f32, None) as u32;

            img.resize(w, h, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };

        // Get image dimensions
        let img_width = img.width();
        let img_height = img.height();

        // Calculate position (center of image)
        let x = get_size(x, self.page_resolution.0 as f32, Some(0.0));
        let y = get_size(y, self.page_resolution.1 as f32, Some(0.0));

        // Adjust position to top-left corner for SVG image element
        let x = x - (img_width as f32 / 2.0);
        let y = y - (img_height as f32 / 2.0);

        // Convert image to PNG and encode as base64
        let mut buffer: Vec<u8> = Vec::new();
        // Use Cursor to wrap the Vec<u8> to implement Seek trait
        let mut cursor = std::io::Cursor::new(&mut buffer);
        img.write_to(&mut cursor, ImageFormat::Png)?;
        let encoded = general_purpose::STANDARD.encode(&buffer);
        let data_url = format!("data:image/png;base64,{}", encoded);

        // Create the image element
        let mut image = Image::new()
            .set("href", data_url)
            .set("x", x)
            .set("y", y)
            .set("width", img_width)
            .set("height", img_height);

        // Apply rotation if specified
        if let Some(rot) = rot {
            // Calculate center of image for rotation
            let center_x = x + (img_width as f32 / 2.0);
            let center_y = y + (img_height as f32 / 2.0);

            // Apply rotation transform around the center
            image = image.set(
                "transform",
                format!("rotate({} {} {})", rot, center_x, center_y),
            );
        }

        // Add the image to the document
        self.document = self.document.clone().add(image);

        Ok(())
    }

    fn write_icon(
        &mut self,
        name: &str,
        x: Option<f32>,
        y: Option<f32>,
        w: Option<f32>,
        h: Option<f32>,
        rot: Option<f32>,
    ) -> Result<(), RenderError> {
        let path = Path::new(name);
        if !path.exists() {
            return Err(RenderError::SvgError(format!(
                "Icon file not found: {}",
                name
            )));
        }

        // Check if it's an SVG file
        if path.extension().map_or(false, |ext| ext != "svg") {
            return Err(RenderError::SvgError(
                "Icon must be an SVG file".to_string(),
            ));
        }

        // Read the SVG file
        let mut file = File::open(path)?;
        let mut svg_content = String::new();
        file.read_to_string(&mut svg_content)?;

        // Encode the SVG content as base64
        let encoded = general_purpose::STANDARD.encode(svg_content.as_bytes());
        let data_url = format!("data:image/svg+xml;base64,{}", encoded);

        // Calculate position and dimensions
        let x = get_size(x, self.page_resolution.0 as f32, Some(0.0));
        let y = get_size(y, self.page_resolution.1 as f32, Some(0.0));
        let w = get_size(w, 100.0, Some(100.0)); // Default width if not specified
        let h = get_size(h, 100.0, Some(100.0)); // Default height if not specified

        // Adjust position to top-left corner for SVG image element
        let x = x - (w / 2.0);
        let y = y - (h / 2.0);

        // Create the image element
        let mut image = Image::new()
            .set("href", data_url)
            .set("x", x)
            .set("y", y)
            .set("width", w)
            .set("height", h);

        // Apply rotation if specified
        if let Some(rot) = rot {
            // Calculate center of image for rotation
            let center_x = x + (w / 2.0);
            let center_y = y + (h / 2.0);

            // Apply rotation transform around the center
            image = image.set(
                "transform",
                format!("rotate({} {} {})", rot, center_x, center_y),
            );
        }

        // Add the image to the document
        self.document = self.document.clone().add(image);

        Ok(())
    }

    fn move_anchor(&mut self, x: f32, y: f32) -> Result<(), RenderError> {
        self.anchor_x = x;
        self.anchor_y = y;
        self.offset_x = 0.0;
        self.offset_y = 0.0;

        Ok(())
    }

    fn start_pin_set(
        &mut self,
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
    ) -> Result<(), RenderError> {
        // Clear existing line settings
        self.line_settings.clear();

        // Convert enums to strings for storage
        let side_str = match side {
            Side::Left => "LEFT",
            Side::Right => "RIGHT",
            Side::Top => "TOP",
            Side::Bottom => "BOTTOM",
        };

        let justify_x_str = match justify_x {
            JustifyX::Left => "LEFT",
            JustifyX::Right => "RIGHT",
            JustifyX::Center => "CENTER",
        };

        let justify_y_str = match justify_y {
            JustifyY::Top => "TOP",
            JustifyY::Bottom => "BOTTOM",
            JustifyY::Center => "CENTER",
        };

        // Store all pin set settings
        self.line_settings.insert("SIDE".into(), side_str.into());
        self.line_settings.insert(
            "PACK".into(),
            (if packed { "PACKED" } else { "UNPACKED" }).into(),
        );
        self.line_settings
            .insert("JUSTIFY X".into(), justify_x_str.into());
        self.line_settings
            .insert("JUSTIFY Y".into(), justify_y_str.into());
        self.line_settings
            .insert("PINWIDTH".into(), pin_width.into());
        self.line_settings
            .insert("GROUPWIDTH".into(), group_width.into());
        self.line_settings
            .insert("LINESTEP".into(), line_step.into());
        self.line_settings
            .insert("LEADER".into(), leader_offset.into());
        self.line_settings.insert("GAP".into(), column_gap.into());
        self.line_settings
            .insert("HSTEP".into(), leader_h_step.into());
        Ok(())
    }

    fn write_pin(
        &mut self,
        wire: Option<WireType>,
        pin_type: Option<PinType>,
        group: &Option<String>,
        attributes: &[String],
    ) -> Result<(), RenderError> {
        if self.line_settings.is_empty() {
            return Err(RenderError::SvgError(
                "Line not setup with prior PINSET!".to_string(),
            ));
        }

        // Print the pin icon and leader line, and get the box offset
        let mut box_offset_x = self.print_pin(pin_type, wire, group)?;

        // Get line height from settings
        let line_height = self
            .line_settings
            .get("LINESTEP")
            .unwrap()
            .parse::<f32>()
            .unwrap_or(10.0);

        // Process each attribute (columns after the pin type, wire, and group)
        for (index, attr) in attributes.iter().enumerate() {
            if index < self.pin_func_types.len() {
                let pin_func = &self.pin_func_types[index];

                if !attr.is_empty() {
                    // Calculate position for the text box
                    let (x, y) = self.get_pin_box_xy(box_offset_x, pin_func, line_height);

                    // Draw the text box
                    self.text_box(
                        x,
                        y,
                        pin_func,
                        attr,
                        &self
                            .line_settings
                            .get("JUSTIFY X")
                            .unwrap_or(&Value::from("CENTER")),
                        &self
                            .line_settings
                            .get("JUSTIFY Y")
                            .unwrap_or(&Value::from("CENTER")),
                    )?;

                    // Increment the box offset for the next box
                    let side = self
                        .line_settings
                        .get("SIDE")
                        .cloned()
                        .unwrap_or(Value::from("LEFT"));
                    box_offset_x = self.inc_offset_x(box_offset_x, &side, pin_func);
                } else if self
                    .line_settings
                    .get("PACK")
                    .unwrap_or(&Value::from("UNPACKED"))
                    .eq_ignore_ascii_case("UNPACKED")
                {
                    // If not packed, still increment the offset for empty boxes
                    let side = self
                        .line_settings
                        .get("SIDE")
                        .cloned()
                        .unwrap_or(Value::from("LEFT"));
                    box_offset_x = self.inc_offset_x(box_offset_x, &side, pin_func);
                }
            }
        }

        // Increment vertical offset for the next pin
        self.offset_y += line_height;

        Ok(())
    }

    fn write_pin_text(
        &mut self,
        wire: Option<WireType>,
        pin_type: Option<PinType>,
        group: &Option<String>,
        theme: &str,
        label: &Option<String>,
        text: &str,
    ) -> Result<(), RenderError> {
        if self.line_settings.is_empty() {
            return Err(RenderError::SvgError(
                "Line not setup with prior PINSET!".to_string(),
            ));
        }

        // Print the pin icon and leader line, and get the box offset
        let mut box_offset_x = self.print_pin(pin_type, wire, group)?;

        // Get line height from settings
        let line_height = self
            .line_settings
            .get("LINESTEP")
            .unwrap()
            .parse::<f32>()
            .unwrap_or(10.0);

        // If a label is provided, draw the first box with the label
        if let Some(label_text) = label {
            if !label_text.is_empty() {
                // Use the first pin function type for the label
                let pin_func = &self.pin_func_types[0]; // First pin function type

                // Calculate position for the text box
                let (x, y) = self.get_pin_box_xy(box_offset_x, pin_func, line_height);

                // Draw the text box with the label
                self.text_box(
                    x,
                    y,
                    pin_func,
                    label_text,
                    &self
                        .line_settings
                        .get("JUSTIFY X")
                        .unwrap_or(&Value::from("CENTER")),
                    &self
                        .line_settings
                        .get("JUSTIFY Y")
                        .unwrap_or(&Value::from("CENTER")),
                )?;

                // Increment the box offset for the text
                let side = self
                    .line_settings
                    .get("SIDE")
                    .cloned()
                    .unwrap_or(Value::from("LEFT"));
                box_offset_x = self.inc_offset_x(box_offset_x, &side, pin_func);
            }
        }

        // If text is provided, draw it after the label
        if !text.is_empty() {
            // Get font settings from the theme
            let font_theme = self.get_font_theme(theme);
            let font = self.get_theme(&font_theme, "FONT", "sans-serif");
            let font_size = self
                .get_theme(&font_theme, "FONT SIZE", "10")
                .parse::<f32>()
                .unwrap_or(10.0);
            let font_color = self.get_theme(&font_theme, "FONT COLOR", "black");
            let font_slant = self.get_theme(&font_theme, "FONT SLANT", "normal");
            let font_bold = self.get_theme(&font_theme, "FONT BOLD", "normal");
            let font_stretch = self.get_theme(&font_theme, "FONT STRETCH", "normal");

            // Calculate position for the text
            let (x, y) = self.get_pin_box_xy(box_offset_x, theme, line_height);

            // Adjust X position for the gap
            let side = self
                .line_settings
                .get("SIDE")
                .cloned()
                .unwrap_or(Value::from("LEFT"));
            let gap = self
                .line_settings
                .get("GAP")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(10.0);
            let x = if side.contains("LEFT") {
                x - gap
            } else {
                x + gap
            };

            // Determine text anchor based on side
            let text_anchor = if side.contains("LEFT") {
                "end"
            } else {
                "start"
            };

            // Create text element
            let text_elem = Text::new("") // TODO this can corrup nodes
                .set("x", x)
                .set("y", y + (line_height / 2.0))
                .set("font-size", font_size)
                .set("font-family", font)
                .set("fill", font_color)
                .set("font-style", font_slant)
                .set("font-weight", font_bold)
                .set("font-stretch", font_stretch)
                .set("text-anchor", text_anchor)
                .add(TextNode::new(text));

            // Add text to document
            self.document = self.document.clone().add(text_elem);
        }

        // Increment vertical offset for the next pin
        self.offset_y += line_height;

        Ok(())
    }

    fn draw_box(
        &mut self,
        theme: &str,
        x: f32,
        y: f32,
        box_width: Option<f32>,
        box_height: Option<f32>,
        x_justify: Option<JustifyX>,
        y_justify: Option<JustifyY>,
        text: &Option<String>,
    ) -> Result<(), RenderError> {
        // Get the box theme name (add BOX_ prefix if not already there)
        let box_theme = if theme.starts_with("BOX_") {
            theme.to_string()
        } else {
            format!("BOX_{}", theme)
        };

        // Convert justify options to strings
        let x_justify_str = match x_justify {
            Some(JustifyX::Left) => "LEFT",
            Some(JustifyX::Right) => "RIGHT",
            Some(JustifyX::Center) => "CENTER",
            None => "CENTER", // Default
        };

        let y_justify_str = match y_justify {
            Some(JustifyY::Top) => "TOP",
            Some(JustifyY::Bottom) => "BOTTOM",
            Some(JustifyY::Center) => "CENTER",
            None => "CENTER", // Default
        };

        // Get width and height from theme or use provided values
        let w = box_width.unwrap_or_else(|| {
            self.get_box_theme(&box_theme, "WIDTH", "0")
                .parse::<f32>()
                .unwrap_or(0.0)
        });

        let h = box_height.unwrap_or_else(|| {
            self.get_box_theme(&box_theme, "HEIGHT", "0")
                .parse::<f32>()
                .unwrap_or(0.0)
        });

        // Draw the text box
        let text_content = text.as_deref().unwrap_or("");
        self.text_box(x, y, &box_theme, text_content, x_justify_str, y_justify_str)?;

        Ok(())
    }

    fn start_text_message(
        &mut self,
        x: Option<f32>,
        y: Option<f32>,
        line_step: Option<f32>,
        font: &Option<String>,
        font_size: Option<f32>,
        x_justify: Option<JustifyX>,
        y_justify: Option<JustifyY>,
    ) -> Result<(), RenderError> {
        // End any previous message
        self.end_message()?;

        // Set message settings
        self.message_settings.insert("Newline".into(), false.into());

        // Set x and y if provided
        if let Some(x_val) = x {
            self.message_settings.insert("X".into(), x_val.into());
            self.message_settings.insert("OffsetX".into(), 0.0.into());
        }

        if let Some(y_val) = y {
            self.message_settings.insert("Y".into(), y_val.into());
            self.message_settings.insert("OffsetY".into(), 0.0.into());
        }

        // Set line step if provided
        if let Some(step) = line_step {
            self.message_settings.insert("LineStep".into(), step.into());
        } else if !self.message_settings.contains_key("LineStep") {
            self.message_settings.insert("LineStep".into(), 15.0.into()); // Default
        }

        // Set font if provided
        if let Some(f) = font {
            self.message_settings
                .insert("Font".into(), f.clone().into());
        } else if !self.message_settings.contains_key("Font") {
            self.message_settings
                .insert("Font".into(), "sans-serif".into()); // Default
        }

        // Set font size if provided
        if let Some(size) = font_size {
            self.message_settings.insert("FontSize".into(), size.into());
        } else if !self.message_settings.contains_key("FontSize") {
            self.message_settings.insert("FontSize".into(), 12.0.into()); // Default
        }

        // Set justify settings
        let x_justify_str = match x_justify {
            Some(JustifyX::Left) => "LEFT",
            Some(JustifyX::Right) => "RIGHT",
            Some(JustifyX::Center) => "CENTER",
            None => "CENTER", // Default
        };

        let y_justify_str = match y_justify {
            Some(JustifyY::Top) => "TOP",
            Some(JustifyY::Bottom) => "BOTTOM",
            Some(JustifyY::Center) => "CENTER",
            None => "CENTER", // Default
        };

        self.message_settings
            .insert("XJustify".into(), x_justify_str.into());
        self.message_settings
            .insert("YJustify".into(), y_justify_str.into());

        // Set text anchor based on x justification
        let text_anchor = match x_justify {
            Some(JustifyX::Left) => "start",
            Some(JustifyX::Right) => "end",
            Some(JustifyX::Center) | None => "middle",
        };

        // Set y shift based on y justification
        let font_size = self
            .message_settings
            .get("FontSize")
            .unwrap()
            .parse::<f32>()
            .unwrap_or(12.0);
        let y_shift = match y_justify {
            Some(JustifyY::Top) => font_size / 2.0,
            Some(JustifyY::Bottom) => -(font_size / 2.0),
            Some(JustifyY::Center) | None => 0.0,
        };

        self.message_settings
            .insert("YShift".into(), y_shift.into());

        // Get font theme
        let font_name = self
            .message_settings
            .get("Font")
            .cloned()
            .unwrap_or(Value::from("sans-serif"));
        let font_theme = self.get_font_theme(&font_name);

        // Create new text element
        let x = self
            .message_settings
            .get("X")
            .unwrap()
            .parse::<f32>()
            .unwrap_or(0.0)
            + self
                .message_settings
                .get("OffsetX")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(0.0);

        let y = self
            .message_settings
            .get("Y")
            .unwrap()
            .parse::<f32>()
            .unwrap_or(0.0)
            + self
                .message_settings
                .get("OffsetY")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(0.0)
            + self
                .message_settings
                .get("YShift")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(0.0);

        let font_size = self
            .message_settings
            .get("FontSize")
            .unwrap()
            .parse::<f32>()
            .unwrap_or(12.0);
        let font_family = self.get_theme(&font_theme, "FONT", "sans-serif");
        let stroke = self.get_theme(&font_theme, "OUTLINE COLOR", "none");
        let fill = self.get_theme(&font_theme, "FONT COLOR", "black");
        let font_style = self.get_theme(&font_theme, "FONT SLANT", "normal");
        let font_weight = self.get_theme(&font_theme, "FONT BOLD", "normal");
        let font_stretch = self.get_theme(&font_theme, "FONT STRETCH", "normal");

        let text_elem = Text::new("") //TODO this can corrupt output
            .set("x", x)
            .set("y", y)
            .set("font-size", font_size)
            .set("font-family", font_family)
            .set("stroke", stroke)
            .set("fill", fill)
            .set("font-style", font_style)
            .set("font-weight", font_weight)
            .set("font-stretch", font_stretch)
            .set("text-anchor", text_anchor);

        self.current_text = Some(text_elem);

        Ok(())
    }

    fn write_text(
        &mut self,
        edge_color: &str,
        color: &str,
        message: &str,
        new_line: bool,
    ) -> Result<(), RenderError> {
        if self.current_text.is_none() {
            return Err(RenderError::SvgError(
                "No multiline text message started!".to_string(),
            ));
        }

        let font_theme = self.get_font_theme(
            &self
                .message_settings
                .get("Font")
                .unwrap_or(&Value::from("sans-serif"))
                .to_string(),
        );

        // Get default color if not specified
        let color = if color.is_empty() {
            self.get_theme(&font_theme, "FONT COLOR", "black")
                .to_owned()
        } else {
            color.to_owned()
        };

        // Get default edge color if not specified
        let edge_color = if edge_color.is_empty() {
            "none"
        } else {
            edge_color
        };

        let mut tspan = TSpan::new("");

        // Check if we need to start a new line
        if self
            .message_settings
            .get("Newline")
            .unwrap()
            .parse()
            .unwrap_or(false)
        {
            // Reset newline flag
            self.message_settings.insert("Newline".into(), false.into());

            // Update Y offset
            let offset_y = self
                .message_settings
                .get("OffsetY")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(0.0);
            let line_step = self
                .message_settings
                .get("LineStep")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(15.0);
            self.message_settings
                .insert("OffsetY".into(), (offset_y + line_step).into());

            // Set position for new line
            let x = self
                .message_settings
                .get("X")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(0.0)
                + self
                    .message_settings
                    .get("OffsetX")
                    .unwrap()
                    .parse::<f32>()
                    .unwrap_or(0.0);

            let y = self
                .message_settings
                .get("Y")
                .unwrap()
                .parse::<f32>()
                .unwrap_or(0.0)
                + self
                    .message_settings
                    .get("OffsetY")
                    .unwrap()
                    .parse::<f32>()
                    .unwrap_or(0.0)
                + self
                    .message_settings
                    .get("YShift")
                    .unwrap()
                    .parse::<f32>()
                    .unwrap_or(0.0);

            tspan = tspan.set("x", x).set("y", y);
        }

        // Set text properties
        tspan = tspan
            .set("stroke", edge_color)
            .set("fill", color)
            .add(TextNode::new(message));

        // Add tspan to current text element
        if let Some(ref mut text) = self.current_text {
            *text = text.clone().add(tspan);
        }

        // Set newline flag if needed
        if new_line {
            self.message_settings.insert("Newline".into(), true.into());
        }

        Ok(())
    }

    fn end_message(&mut self) -> Result<(), RenderError> {
        if let Some(text) = self.current_text.take() {
            self.document = self.document.clone().add(text);
        }
        Ok(())
    }

    fn get_theme(&self, font_theme: &str, arg_1: &str, arg_2: &str) -> &str {
        todo!()
    }

    fn get_font_theme(&self, font_name: &str) -> String {
        todo!()
    }

    fn text_box(
        &self,
        x: f32,
        y: f32,
        box_theme: &str,
        text_content: &str,
        x_justify_str: &str,
        y_justify_str: &str,
    ) -> Result<f32, RenderError> {
        todo!()
    }

    fn get_box_theme(&self, box_theme: &str, arg_1: &str, arg_2: &str) -> String {
        todo!()
    }

    fn get_pin_box_xy(&self, box_offset_x: f32, theme: &str, line_height: f32) -> (f32, f32) {
        todo!()
    }

    fn inc_offset_x(&self, box_offset_x: f32, side: &str, pin_func: &str) -> f32 {
        todo!()
    }

    fn print_pin(
        &self,
        pin_type: Option<PinType>,
        wire: Option<WireType>,
        group: &Option<String>,
    ) -> Result<f32, RenderError> {
        todo!()
    }

    // Helper methods
}

fn font_boldness_to_string(bold: FontBoldness) -> String {
    todo!()
}

fn get_size(x: Option<f32>, page_resolution: f32, some: Option<f64>) -> f32 {
    todo!()
}

fn font_slant_to_string(default: FontSlant) -> String {
    todo!()
}

fn font_stretch_to_string(default: FontStretch) -> String {
    todo!()
}
