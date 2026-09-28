# pinout

[![Crates.io](https://img.shields.io/crates/v/pinout.svg)](https://crates.io/crates/pinout)
[![Documentation](https://docs.rs/pinout/badge.svg)](https://docs.rs/pinout)
[![License](https://img.shields.io/github/license/orhanbalci/pinout.svg)](https://github.com/orhanbalci/pinout/blob/master/LICENSE)

Pinout diagrams for microcontrollers, development boards and chips. Boards
are described in the YAML format of [pinoutleaf](https://github.com/splitbrain/pinoutleaf)
and drawn as true-to-scale SVG, as colored terminal output, or edited live in
an interactive terminal UI.

![front view](https://raw.githubusercontent.com/orhanbalci/pinout/v0.2.0/assets/tui/front.png)

## ✨ Features

- **pinoutleaf-compatible YAML**: pins are lists of `label:type` strings; existing pinoutleaf files work as they are
- **One model, many outputs**: SVG, terminal, JSON and YAML are all rendered from the same board model
- **True-to-scale SVG**: pads sit on the 0.1" raster and the SVG is sized in millimeters, so a print at 100% fits the real board
- **Board photos**: front and back images are embedded into the SVG, and the back view is mirrored for you
- **Interactive editor**: a vim-like YAML editor next to a preview that follows every change

## 📦 Installation

The `pinout` command line tool and terminal UI:

```bash
cargo install pinout
```

The library, without the dependencies of the command line tool:

```toml
[dependencies]
pinout = { version = "0.2", default-features = false }
```

## 🚀 Quick Start

Describe the board in `my_board.yaml`:

```yaml
title: "ESP32 C3 Super Mini"

# dimensions counted in pins
width: 7
height: 8

pins:
  left:
    - [ "GPIO5:gpio", "A5:analog", "MISO:spi" ]
    - [ "GPIO6:gpio", "MOSI:spi" ]
    - [ "GPIO7:gpio", "SS:spi" ]
    - [ "GPIO8:gpio", "SDA:i2c" ]
    - [ "GPIO9:gpio", "SCL:i2c" ]
    - [ "GPIO10:gpio" ]
    - [ "GPIO20:gpio", "RX:uart" ]
    - [ "GPIO21:gpio", "TX:uart" ]
  right:
    - [ "5V:power" ]
    - [ "GND:gnd" ]
    - [ "3V3:power" ]
    - [ "GPIO4:gpio", "A4:analog", "SCK:spi" ]
```

Then look at it in the terminal, edit it interactively, or write the SVG for
the front and the back:

```bash
pinout my_board.yaml
pinout my_board.yaml --tui
pinout my_board.yaml -o my_board.svg
pinout my_board.yaml --back -o my_board.back.svg
```

## 🖥️ Interactive Editor

`--tui` (or `-i`) opens the description in a terminal UI built with
[ratatui](https://ratatui.rs) and [edtui](https://github.com/preiter93/edtui).
Every edit is parsed right away; while the text has errors the other tabs keep
the last valid version and the status line shows what is wrong.

![editor](https://raw.githubusercontent.com/orhanbalci/pinout/v0.2.0/assets/tui/editor.png)

![pins](https://raw.githubusercontent.com/orhanbalci/pinout/v0.2.0/assets/tui/pins.png)

![columns](https://raw.githubusercontent.com/orhanbalci/pinout/v0.2.0/assets/tui/columns.png)

| Tab | Keys |
| --- | --- |
| `F1` Editor | vim keys (`i`, `Esc`, `dd`, `ciw`, `u`, ...) and `:w`, `:q`, `:q!`, `:wq`, `:x` |
| `F2` Front, `F3` Back | `hjkl` scroll, `v` choose the label columns to show, `f` fit labels to the width, `c` compact rows |
| `F4` Pins | `j`/`k` select a pin, `h`/`l` scroll the label columns |
| `F5` Notes | `j`/`k` scroll |

`Tab` and `Shift-Tab` switch tabs, `Ctrl-s` saves and `q` quits, asking once
more when there are unsaved changes.

## 🔧 Command Line

```bash
pinout <input> [options]
```

The input is a `.yaml`, `.yml` or `.json` board description, or a legacy `.csv`
file that is converted on the fly. The output format follows the extension of
`-o` and defaults to the terminal.

<details>
<summary><b>All options</b></summary>

| Option | Description |
| --- | --- |
| `-f, --format <term\|svg\|json\|yaml>` | Output format, overrides the `-o` extension |
| `-o, --output <file>` | Write to a file instead of stdout |
| `-i, --tui` | Open the interactive editor |
| `-b, --back` | Render the back of the board |
| `-t, --types <types>` | Terminal: only show labels of these types, e.g. `gpio,power,gnd` |
| `-n, --notes` | Terminal: print the board's notes |
| `-c, --compact` | Terminal: no gap between pins |
| `-w, --width <n>` | Terminal: maximum width, the terminal width by default; long labels are shortened from the middle until the diagram fits |
| `-m, --max-label <n>` | Terminal: shorten labels longer than `n` characters, e.g. `GPIO36` to `GP…36` |
| `-p, --packed` | Terminal: flow labels next to each other instead of aligning them in columns |
| `--no-color` | Terminal: plain text; `NO_COLOR` and `CLICOLOR_FORCE` are honored too |

</details>

## 📚 Library

```rust
use pinout::model::Pinout;
use pinout::renderer::leaf::{render_svg, SvgOptions};
use pinout::renderer::term::{render_terminal, TermOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pinout = Pinout::from_path("my_board.yaml")?;

    std::fs::write("my_board.svg", render_svg(&pinout, &SvgOptions::default())?)?;
    print!("{}", render_terminal(&pinout, &TermOptions::default())?);
    println!("{}", pinout.to_json()?);

    Ok(())
}
```

## 📝 YAML Format

The format is the one of [pinoutleaf](https://github.com/splitbrain/pinoutleaf#configuration-syntax);
JSON files use the same structure.

- `title`: title of the diagram
- `width`, `height`: board size in pins on the 0.1" raster; when left out they follow the longest rows
- `pins`: up to four rows, `left`, `right`, `top` and `bottom`. Each pin is a list of `label:type` strings from the pin outwards, and an empty entry leaves a position unused. An empty string (`""`) keeps a label column free so the labels after it stay aligned. A suffix that is not a known type stays part of the label, so `ADC1:0` needs no escaping
- `types`: custom types with `label` (legend text), `bgcolor` and `fgcolor`. The built-in `gpio`, `power`, `gnd`, `i2c`, `uart`, `spi` and `analog` can be used directly or restyled; labels without a type use `default`
- `image`: board photos for the `front` and `back`: `src` plus optional `top`, `left`, `right` and `bottom` offsets in 1/100 mm, `opacity` and `grayscale`
- `offsets`: move a row of pins inwards by a number of pins
- `notes`: text blocks with an optional `title` and a list of `lines`, shown in the Notes tab and with `--notes`. This is an extension that pinoutleaf ignores

```yaml
types:
  motor:
    label: Output
    bgcolor: "#439ED6"
    fgcolor: "#ffffff"

notes:
  - title: Features
    lines:
      - "WiFi 2.4GHz 802.11 b/g/n"
      - "Bluetooth 5 LE"
```

## 🧩 Examples

- [`ATtiny85.yaml`](ATtiny85.yaml): the ATtiny85 in its DIP-8 package, after the pin configuration and Port B alternate functions of the ATtiny25/45/85 datasheet. Functions of one kind share a column, and notes describe the chip
- [`ESP32-MAXIO.yaml`](ESP32-MAXIO.yaml): a large ESP32 and SAML21 board, converted from the legacy [`ESP32-MAXIO.csv`](ESP32-MAXIO.csv)

## 🗄️ Legacy CSV Format

Before the YAML format, diagrams were described in CSV files that place pin
sets freely on a page. `cargo run --example main input.csv output.svg` still
renders them with the original SVG renderer, and the `pinout` tool converts them
to YAML:

```bash
pinout ESP32-MAXIO.csv -o ESP32-MAXIO.yaml
```

The conversion takes the main left and right pin sets as the board, guesses
label types from their names (`GPIO21` is GPIO, `ADC1:0` analog, `HSPI:CLK`
SPI, ...), keeps messages as notes and reports everything else as a warning.

<details>
<summary><b>CSV structure and commands</b></summary>

A file has a setup phase that defines themes and styles, and a draw phase,
started by `DRAW`, that places the elements:

```csv
# Setup phase
LABELS,DEFAULT,TYPE,GROUP,Pin Name,Function 1,Function 2
BORDER COLOR,black
FILL COLOR,white,white,white,lightblue,yellow
FONT,Arial
FONT SIZE,12

TYPE,IO,blue,1
TYPE,Input,green,1
TYPE,Output,red,1
GROUP,IO,lightblue,0.5
GROUP,Input,lightgreen,0.5
GROUP,Output,lightyellow,0.5

# Draw phase
DRAW
ANCHOR,50,100
PINSET,LEFT,PACKED,CENTER,CENTER,20,80,100,10,5,2
PIN,1,VDD,Output,,3.3V Power
PIN,2,GND,Output,,Ground
```

Setup phase commands:

- `LABELS`: the label columns of the pins
- `BORDER COLOR`, `BORDER WIDTH`, `BORDER OPACITY`: borders of the label boxes
- `FILL COLOR`, `OPACITY`: fill of the label boxes, per column
- `FONT`, `FONT SIZE`, `FONT COLOR`, `FONT SLANT`, `FONT BOLD`, `FONT STRETCH`, `FONT OUTLINE`, `FONT OUTLINE THICKNESS`: text style, per column
- `TYPE`, `WIRE`, `GROUP`: pin types, wire types and pin groups with their colors
- `BOX`, `TEXT FONT`: named box and text themes
- `PAGE`, `DPI`: page size (`A3-L`, `A4-P`, ...) and resolution

Draw phase commands:

- `ANCHOR`: origin of the next pin set
- `PINSET`: starts a pin set with its side and layout
- `PIN`, `PINTEXT`: a pin with its labels, or with a label and free text
- `IMAGE`, `ICON`: raster images and SVG icons
- `BOX`: a styled box
- `MESSAGE`, `TEXT`, `END MESSAGE`: blocks of text
- `GOOGLEFONT`: web fonts to load

</details>

## 🙏 Acknowledgments

- The board description format and SVG layout come from [pinoutleaf](https://github.com/splitbrain/pinoutleaf) by Andreas Gohr
- The CSV format follows [GenPinoutSVG](https://github.com/stevenj/GenPinoutSVG)
- The interactive editor is built on [ratatui](https://ratatui.rs) and [edtui](https://github.com/preiter93/edtui)

## 📝 License

Licensed under MIT License ([LICENSE](LICENSE)).

### 🚧 Contributions

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the MIT license, shall be
licensed as above, without any additional terms or conditions.

The images in this README are drawn by
[`scripts/readme_images.py`](scripts/readme_images.py); its header explains how
to regenerate them.
