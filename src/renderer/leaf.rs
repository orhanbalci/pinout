//! SVG renderer for the [`Pinout`] model, following pinoutleaf's layout.
//!
//! All coordinates are in 1/100 mm and the SVG declares its size in mm, so a
//! print at 100% puts every pad on top of the real pin.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;

use crate::model::{extension, BoardImage, Edge, Label, Pinout, PinoutError};

/// Radius of a pad.
const PIN_SIZE: f64 = 60.0;
/// Standard 0.1" pin raster.
const PIN_SPACE: f64 = 254.0;
const PADDING: f64 = 100.0;
const FONT_SIZE: f64 = 150.0;
const CORNERS: f64 = 30.0;
/// Width of a monospace character relative to the font size.
const CHAR_WIDTH: f64 = 0.6;
const LEGEND_STROKE: f64 = 10.0;
const FONT_FAMILY: &str = "'Roboto Mono', 'DejaVu Sans Mono', monospace";

#[derive(Debug, Clone, Default)]
pub struct SvgOptions {
    /// Draw the back of the board instead of the front.
    pub back: bool,
    /// Directory that relative image paths are resolved against.
    pub base_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl BBox {
    fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    fn right(&self) -> f64 {
        self.x + self.w
    }

    fn bottom(&self) -> f64 {
        self.y + self.h
    }

    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    fn union(self, other: BBox) -> BBox {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        BBox::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    fn translate(self, dx: f64, dy: f64) -> BBox {
        BBox::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

fn union_all(boxes: impl IntoIterator<Item = BBox>) -> Option<BBox> {
    boxes.into_iter().reduce(BBox::union)
}

/// Render the front (or back) of a board as a standalone SVG document.
pub fn render_svg(pinout: &Pinout, options: &SvgOptions) -> Result<String, PinoutError> {
    let pinout = if options.back {
        pinout.flipped()
    } else {
        pinout.clone()
    };

    let mut layout = String::new();
    let mut layout_boxes = Vec::new();

    // The board goes first so pads and labels are drawn on top of it
    let (board, board_box) = board(&pinout, options)?;
    layout.push_str(&board);
    layout_boxes.push(board_box);

    for edge in Edge::ALL {
        for (index, pin) in pinout.row(edge).iter().enumerate() {
            if pin.is_empty() {
                continue;
            }
            let (x, y) = pin_position(&pinout, edge, index);
            let _ = write!(
                layout,
                r#"<circle cx="{}" cy="{}" r="{}" fill="gold"/>"#,
                num(x),
                num(y),
                num(PIN_SIZE)
            );
            let mut last = BBox::new(x - PIN_SIZE, y - PIN_SIZE, PIN_SIZE * 2.0, PIN_SIZE * 2.0);
            layout_boxes.push(last);
            // Blank labels only align columns in the terminal
            for (n, label) in pin.iter().filter(|l| !l.text.is_empty()).enumerate() {
                // The first label keeps more distance to the pad
                let padding = if n == 0 { PADDING * 3.0 } else { PADDING };
                let (svg, bbox) = pin_label(&pinout, label, edge, last, padding);
                layout.push_str(&svg);
                layout_boxes.push(bbox);
                last = bbox;
            }
        }
    }
    let layout_box = union_all(layout_boxes).unwrap_or(BBox::new(0.0, 0.0, 0.0, 0.0));

    let title = format!(
        "{}{}",
        pinout.title,
        if options.back { "  (back)" } else { " (front)" }
    );
    let title_box = BBox::new(0.0, 0.0, text_width(&title), FONT_SIZE);

    // The legend sits beside the board, bottom aligned, on the outer side
    let (legend, legend_box) = legend(&pinout);
    let mut root_box = layout_box.union(title_box);
    let mut legend_offset = (0.0, 0.0);
    if let Some(legend_box) = legend_box {
        root_box = root_box.union(legend_box);
        let x = if options.back {
            layout_box.x - legend_box.w + LEGEND_STROKE - PADDING * 3.0
        } else {
            layout_box.right() + PADDING * 3.0
        };
        legend_offset = (x, root_box.bottom() - legend_box.h);
        root_box = layout_box
            .union(title_box)
            .union(legend_box.translate(legend_offset.0, legend_offset.1));
    }

    let title_offset = (root_box.x, root_box.y - PADDING - FONT_SIZE);
    let total = root_box.union(title_box.translate(title_offset.0, title_offset.1));

    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{wmm}mm" height="{hmm}mm">"#,
        w = num(total.w),
        h = num(total.h),
        wmm = num(total.w / 100.0),
        hmm = num(total.h / 100.0),
    );
    svg.push_str(
        r#"<defs><filter id="grayscale"><feColorMatrix type="saturate" values="0"/></filter></defs>"#,
    );
    let _ = write!(
        svg,
        r#"<g id="root" transform="translate({} {})">"#,
        num(-total.x),
        num(-total.y)
    );
    let _ = write!(svg, "<g>{layout}</g>");
    let _ = write!(
        svg,
        r#"<g transform="translate({} {})">{}</g>"#,
        num(title_offset.0),
        num(title_offset.1),
        text(0.0, 0.0, &title, "#000000")
    );
    if legend_box.is_some() {
        let _ = write!(
            svg,
            r#"<g transform="translate({} {})">{legend}</g>"#,
            num(legend_offset.0),
            num(legend_offset.1)
        );
    }
    svg.push_str("</g></svg>\n");
    Ok(svg)
}

fn pin_position(pinout: &Pinout, edge: Edge, index: usize) -> (f64, f64) {
    let (width, height) = pinout.size();
    let offsets = pinout.offsets;
    let i = index as f64;
    match edge {
        Edge::Left => (offsets.left as f64 * PIN_SPACE, i * PIN_SPACE),
        Edge::Right => (
            (width - 1 - offsets.right) as f64 * PIN_SPACE,
            i * PIN_SPACE,
        ),
        Edge::Top => (i * PIN_SPACE, offsets.top as f64 * PIN_SPACE),
        Edge::Bottom => (
            i * PIN_SPACE,
            (height - 1 - offsets.bottom) as f64 * PIN_SPACE,
        ),
    }
}

/// A rounded label next to `last`, rotated upright for the top and bottom
/// rows. Returns the SVG and the label's outline on the page.
fn pin_label(
    pinout: &Pinout,
    label: &Label,
    edge: Edge,
    last: BBox,
    padding: f64,
) -> (String, BBox) {
    let style = pinout.resolve_type(label.kind.as_deref());
    let w = text_width(&label.text) + CORNERS * 2.0;
    let h = FONT_SIZE + CORNERS * 2.0;
    let (last_cx, last_cy) = last.center();

    let (cx, cy, rotated) = match edge {
        Edge::Left => (last.x - padding - w / 2.0, last_cy, false),
        Edge::Right => (last.right() + padding + w / 2.0, last_cy, false),
        Edge::Top => (last_cx, last.y - padding - w / 2.0, true),
        Edge::Bottom => (last_cx, last.bottom() + padding + w / 2.0, true),
    };
    let (tx, ty) = (cx - w / 2.0, cy - h / 2.0);

    let mut transform = format!("translate({} {})", num(tx), num(ty));
    let bbox = if rotated {
        let _ = write!(transform, " rotate(270 {} {})", num(w / 2.0), num(h / 2.0));
        BBox::new(cx - h / 2.0, cy - w / 2.0, h, w)
    } else {
        BBox::new(tx, ty, w, h)
    };

    let svg = format!(
        r#"<g transform="{transform}"><rect x="0" y="0" width="{}" height="{}" fill="{}" rx="{c}" ry="{c}"/>{}</g>"#,
        num(w),
        num(h),
        escape(&style.bgcolor),
        text(CORNERS, CORNERS, &label.text, &style.fgcolor),
        c = num(CORNERS),
    );
    (svg, bbox)
}

/// The board outline: a photo when one is configured, a green PCB otherwise.
fn board(pinout: &Pinout, options: &SvgOptions) -> Result<(String, BBox), PinoutError> {
    let (width, height) = pinout.size();
    let padding = PIN_SPACE / 2.0;
    let outline = BBox::new(
        -padding,
        -padding,
        (width - 1) as f64 * PIN_SPACE + padding * 2.0,
        (height - 1) as f64 * PIN_SPACE + padding * 2.0,
    );

    let front = pinout.image.front.as_ref().filter(|i| !i.src.is_empty());
    let (svg, bbox) = match front {
        Some(image) => {
            let bbox = BBox::new(
                outline.x + image.left,
                outline.y + image.top,
                outline.w - image.left - image.right,
                outline.h - image.top - image.bottom,
            );
            let href = embed_image(&image.src, options.base_dir.as_deref())?;
            let filter = if image.grayscale {
                r#" filter="url(#grayscale)""#
            } else {
                ""
            };
            let svg = format!(
                r#"<image x="{}" y="{}" width="{}" height="{}" href="{}" preserveAspectRatio="none" opacity="{}"{filter}/>"#,
                num(bbox.x),
                num(bbox.y),
                num(bbox.w),
                num(bbox.h),
                escape(&href),
                num(image.opacity),
            );
            (svg, bbox)
        }
        None => (
            format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#558f0e" rx="{c}" ry="{c}"/>"##,
                num(outline.x),
                num(outline.y),
                num(outline.w),
                num(outline.h),
                c = num(CORNERS),
            ),
            outline,
        ),
    };

    // Front and back diagrams must be the same size to fold them on top of
    // each other, so reserve the space the other side's image sticks out.
    let (top, left, right, bottom) = image_padding(&pinout.image.front, &pinout.image.back);
    let padded = BBox::new(
        bbox.x - left,
        bbox.y - top,
        bbox.w + left + right,
        bbox.h + top + bottom,
    );
    Ok((svg, padded))
}

fn image_padding(front: &Option<BoardImage>, back: &Option<BoardImage>) -> (f64, f64, f64, f64) {
    let has_src = |image: &Option<BoardImage>| image.as_ref().is_some_and(|i| !i.src.is_empty());
    if !has_src(front) && !has_src(back) {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let default = BoardImage::default();
    let front = front.as_ref().unwrap_or(&default);
    let back = back.as_ref().unwrap_or(&default);
    // Only outward (negative) offsets grow the board
    let diff = |front: f64, back: f64| {
        if (front > 0.0 && back > 0.0) || front < back {
            0.0
        } else {
            (front - back).abs()
        }
    };
    (
        diff(front.top, back.top),
        diff(front.left, back.left),
        diff(front.right, back.right),
        diff(front.bottom, back.bottom),
    )
}

/// Local PNG and JPEG files are embedded as data URLs so the SVG stays
/// self-contained; URLs are linked.
fn embed_image(src: &str, base_dir: Option<&Path>) -> Result<String, PinoutError> {
    if src.starts_with("data:") || src.starts_with("http://") || src.starts_with("https://") {
        return Ok(src.to_string());
    }
    let path = match base_dir {
        Some(dir) => dir.join(src),
        None => PathBuf::from(src),
    };
    let mime = match extension(&path).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => {
            return Err(PinoutError::Invalid(format!(
                "image {src} must be a PNG or JPEG file"
            )));
        }
    };
    let data = std::fs::read(&path)
        .map_err(|e| PinoutError::Invalid(format!("cannot read image {}: {e}", path.display())))?;
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(data)
    ))
}

/// Color swatches for every used type, in a rounded box. The box is drawn at
/// the origin and returned with its outline including the stroke.
fn legend(pinout: &Pinout) -> (String, Option<BBox>) {
    let types = pinout.used_types();
    if types.is_empty() {
        return (String::new(), None);
    }

    let mut items = String::new();
    let mut boxes = Vec::new();
    for (index, kind) in types.iter().enumerate() {
        let y = PADDING + index as f64 * (FONT_SIZE + PADDING);
        let _ = write!(
            items,
            r#"<g transform="translate({} {})"><rect x="0" y="0" width="{s}" height="{s}" fill="{}" rx="{c}" ry="{c}"/>{}</g>"#,
            num(PADDING),
            num(y),
            escape(&kind.bgcolor),
            text(FONT_SIZE + PADDING, 0.0, &kind.label, "#000000"),
            s = num(FONT_SIZE),
            c = num(CORNERS),
        );
        boxes.push(BBox::new(
            PADDING,
            y,
            FONT_SIZE + PADDING + text_width(&kind.label),
            FONT_SIZE,
        ));
    }

    let items_box = union_all(boxes).unwrap_or(BBox::new(0.0, 0.0, 0.0, 0.0));
    let background = BBox::new(
        items_box.x - PADDING,
        items_box.y - PADDING,
        items_box.w + PADDING * 2.0,
        items_box.h + PADDING * 2.0,
    );
    let svg = format!(
        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#ffffff" stroke="#cccccc" stroke-width="{}" rx="{c}" ry="{c}"/>{items}"##,
        num(background.x),
        num(background.y),
        num(background.w),
        num(background.h),
        num(LEGEND_STROKE),
        c = num(CORNERS),
    );
    let stroke = LEGEND_STROKE / 2.0;
    let outline = BBox::new(
        background.x - stroke,
        background.y - stroke,
        background.w + LEGEND_STROKE,
        background.h + LEGEND_STROKE,
    );
    (svg, Some(outline))
}

fn text(x: f64, y: f64, content: &str, fill: &str) -> String {
    format!(
        r#"<text x="{}" y="{}" fill="{}" dominant-baseline="hanging" font-family="{}" font-size="{}" xml:space="preserve">{}</text>"#,
        num(x),
        num(y),
        escape(fill),
        escape(FONT_FAMILY),
        num(FONT_SIZE),
        escape(content)
    )
}

/// Estimated width of a line of text; the font is monospaced.
fn text_width(text: &str) -> f64 {
    text.chars().count() as f64 * FONT_SIZE * CHAR_WIDTH
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn num(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pinout() -> Pinout {
        Pinout::from_yaml_str(
            r#"
title: Demo
width: 3
height: 2
pins:
  left:
    - [ "GPIO5:gpio", "A5:analog" ]
    - [ "R&D" ]
  right:
    - [ "5V:power" ]
  top:
    - [ "SWDIO:gpio" ]
"#,
        )
        .unwrap()
    }

    #[test]
    fn draws_pads_labels_and_legend() {
        let svg = render_svg(&pinout(), &SvgOptions::default()).unwrap();
        assert!(svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg""#));
        assert_eq!(svg.matches("<circle").count(), 4);
        assert!(svg.contains(">Demo (front)</text>"));
        assert!(svg.contains(">R&amp;D</text>"));
        assert!(svg.contains(r##"fill="#79bc3c""##));
        // Top row labels stand upright
        assert!(svg.contains("rotate(270"));
        for legend in [">Analog<", ">GPIO<", ">Pin<", ">Power<"] {
            assert!(svg.contains(legend), "{legend}");
        }
    }

    #[test]
    fn back_side_mirrors_rows() {
        let svg = render_svg(
            &pinout(),
            &SvgOptions {
                back: true,
                ..SvgOptions::default()
            },
        )
        .unwrap();
        assert!(svg.contains(">Demo  (back)</text>"));
        // 5V moved to the left column at x = 0
        let five_volt = svg.find(">5V<").unwrap();
        let group = svg[..five_volt].rfind("<g transform=\"translate(").unwrap();
        assert!(
            svg[group..five_volt].contains("translate(-"),
            "{}",
            &svg[group..five_volt]
        );
    }

    #[test]
    fn rejects_missing_images() {
        let mut pinout = pinout();
        pinout.image.front = Some(BoardImage {
            src: "missing.png".to_string(),
            ..BoardImage::default()
        });
        assert!(render_svg(&pinout, &SvgOptions::default()).is_err());
    }
}
