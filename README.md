# Pinout

A Rust library and command-line tool for generating pinout diagrams of microcontrollers, development boards, and electronic components. Boards are described in the YAML format of [pinoutleaf](https://github.com/splitbrain/pinoutleaf) and rendered as SVG, as colored terminal output, or converted to JSON.

## Features

- **pinoutleaf-compatible YAML**: Describe the pins of a board as `label:type` lists; existing pinoutleaf files work as they are
- **One model, many outputs**: SVG, terminal (ANSI truecolor), JSON and YAML are rendered from the same board model
- **True-to-scale SVG**: Pads sit on the 0.1" raster and the SVG is sized in millimeters, so a print at 100% matches the real board
- **Board photos**: Front and back images are embedded into the SVG, with the back view mirrored automatically
- **Terminal preview**: Colored label chips, a board with pads, a color legend and optional notes, right in the terminal
- **Legacy CSV import**: The older CSV descriptions convert to the YAML model

## Installation

### From Source

```bash
git clone https://github.com/orhanbalci/pinout
cd pinout
cargo build --release
```

### As a Library

Add this to your `Cargo.toml`:

```toml
[dependencies]
pinout = "0.1.0"
```

## Quick Start

1. **Describe the board** (`my_board.yaml`):
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

2. **Look at it in the terminal**:
```bash
cargo run --example pinout -- my_board.yaml
```

3. **Generate the SVG**:
```bash
cargo run --example pinout -- my_board.yaml -o my_board.svg
cargo run --example pinout -- my_board.yaml --back -o my_board.back.svg
```

## Usage

### Command Line Tool

```bash
cargo run --example pinout -- <input> [options]
```

The input is a `.yaml`, `.yml` or `.json` board description, or a legacy `.csv` file which is converted on the fly. The output format follows the extension of `-o`, and defaults to the terminal.

| Option | Description |
| --- | --- |
| `-f, --format <term\|svg\|json\|yaml>` | Output format, overrides the `-o` extension |
| `-o, --output <file>` | Write to a file instead of stdout |
| `-b, --back` | Render the back of the board |
| `-t, --types <types>` | Terminal: only show labels of these types, e.g. `gpio,power,gnd` |
| `-n, --notes` | Terminal: print the board's notes (feature lists, warnings) |
| `-c, --compact` | Terminal: no gap between pins |
| `-w, --width <n>` | Terminal: maximum width (defaults to the terminal width); long labels are shortened from the middle until the diagram fits |
| `-m, --max-label <n>` | Terminal: shorten labels longer than `n` characters, e.g. `GPIO36` to `GP…36` |
| `-p, --packed` | Terminal: flow labels next to each other instead of aligning the n-th label of every pin in one column |
| `--no-color` | Terminal: plain text; `NO_COLOR` and `CLICOLOR_FORCE` are honored too |

Convert a legacy CSV description to YAML:

```bash
cargo run --example pinout -- ESP32-MAXIO.csv -o ESP32-MAXIO.yaml
```

### As a Library

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

## YAML Format

The format is the one of [pinoutleaf](https://github.com/splitbrain/pinoutleaf#configuration-syntax); JSON files use the same structure.

- `title`: Title of the diagram
- `width`, `height`: Board size in pins on the 0.1" raster. When left out, they follow the longest rows
- `pins`: Up to four rows, `left`, `right`, `top` and `bottom`. Each pin is a list of `label:type` strings ordered from the pin outwards; an empty entry leaves a position unused. An empty string (`""`) keeps a label column free, so the following labels stay aligned in the terminal. A suffix that is not a known type stays part of the label, so `ADC1:0` needs no escaping
- `types`: Custom types with `label` (legend text), `bgcolor` and `fgcolor`. The built-in types `gpio`, `power`, `gnd`, `i2c`, `uart`, `spi` and `analog` can be used directly or restyled; labels without a type use `default`
- `image`: Board photos for the `front` and `back` (`src` plus optional `top`, `left`, `right`, `bottom` offsets in 1/100 mm, `opacity` and `grayscale`)
- `offsets`: Move a row of pins inwards by a number of pins
- `notes`: Free text blocks with an optional `title` and a list of `lines`, printed in the terminal with `--notes`. This is an extension; pinoutleaf ignores it

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

## Legacy CSV Format

The CSV format below predates the YAML format. `cargo run --example main input.csv output.svg` still renders it with the original SVG renderer, and the `pinout` tool converts it to the YAML model: the main left and right pin sets become the board, label types are guessed from their names (`GPIO21` is GPIO, `ADC1:0` analog, `HSPI:CLK` SPI, ...), messages become notes, and everything else is reported as a warning.


The CSV format uses a two-phase approach for defining pinout diagrams:

### 1. Setup Phase
Defines themes, styling, and configuration that applies to the entire diagram.

### 2. Draw Phase
Contains the actual drawing commands. Triggered by the `DRAW` command.

### Basic Structure

```csv
# Setup Phase - Define themes and styles
LABELS,DEFAULT,TYPE,GROUP,Pin Name,Function 1,Function 2
BORDER COLOR,black
FILL COLOR,white,white,white,lightblue,yellow
FONT,Arial
FONT SIZE,12

# Define pin types and groups
TYPE,IO,blue,1
TYPE,Input,green,1
TYPE,Output,red,1
GROUP,IO,lightblue,0.5
GROUP,Input,lightgreen,0.5
GROUP,Output,lightyellow,0.5

# Draw Phase - Start rendering
DRAW
ANCHOR,50,100
PINSET,LEFT,PACKED,CENTER,CENTER,20,80,100,10,5,2
PIN,1,VDD,Output,,3.3V Power
PIN,2,GND,Output,,Ground
# ... more pins
```

## Command Reference

### Setup Phase Commands

#### Theme Definition
- `LABELS` - Define pin labels and column structure
- `BORDER COLOR` - Set border colors for different pin types
- `FILL COLOR` - Set fill colors for pin boxes
- `FONT` - Define font families
- `FONT SIZE` - Set font sizes
- `FONT COLOR` - Set text colors
- `OPACITY` - Set transparency levels

#### Styling Commands
- `BORDER WIDTH` - Border line thickness
- `BORDER OPACITY` - Border transparency
- `TYPE` - Define pin types (IO, Input, Output)
- `WIRE` - Define wire types and colors
- `GROUP` - Define pin groups with custom styling
- `BOX` - Define box themes and dimensions

#### Page Setup
- `PAGE` - Set page size ("A3-L", "A4-P", etc.)
- `DPI` - Set resolution for rendering

### Draw Phase Commands

#### Layout Commands
- `ANCHOR` - Set drawing origin point
- `PINSET` - Start a new set of pins with layout parameters
- `PIN` - Add individual pins with labels and properties
- `PINTEXT` - Add text labels to pins

#### Visual Elements
- `IMAGE` - Embed raster images
- `ICON` - Add SVG icons
- `BOX` - Draw styled boxes
- `MESSAGE` - Add text messages
- `TEXT` - Add styled text elements

## Examples

The repository includes example CSV files demonstrating different features:

### ATtiny85
`ATtiny85.yaml` describes the ATtiny85 in its DIP-8 package, following the pin configuration and Port B alternate functions of the ATtiny25/45/85 datasheet. It keeps functions of the same kind in one column with blank labels and carries notes about the chip:

```bash
cargo run --example pinout -- ATtiny85.yaml --notes
```

### ESP32 Development Board
`ESP32-MAXIO.yaml` is the YAML conversion of `ESP32-MAXIO.csv`. The CSV file is a complete example of the legacy format showing:
- Complex pin labeling with multiple functions per pin
- Custom color schemes for different pin types and groups
- Image embedding for board visualization
- Professional styling and layout
- Advanced features like wire types and pin grouping

### Pin Types and Groups

```csv
# Define pin types with colors
TYPE,IO,black,1
TYPE,Input,blue,1  
TYPE,Output,red,1

# Define groups with custom styling
GROUP,Power,black,0
GROUP,Analog,green,0.5

# Use in pin definitions
PIN,1,VCC,Output,Power,3.3V Supply
PIN,2,A0,Input,Analog,Analog Input 0
```

## API Documentation

### Core Types

- `Command` - Enumeration of all supported CSV commands
- `Phase` - Setup or Draw phase indicator  
- `PinType` - IO, Input, Output pin classifications
- `WireType` - Digital, PWM, Analog wire types
- `Side` - Left, Right, Top, Bottom positioning

### Parser Module

- `parse_csv_file(path)` - Parse CSV file into command list
- `Document` - Higher-level document representation with validation

### Renderer Module

- `generate_svg(commands, output_path)` - Render commands to SVG file
- `SvgRenderer` - Low-level SVG rendering engine with theming support

## Error Handling

The library provides comprehensive error handling:

- `ParserError` - CSV parsing and validation errors
- `RenderError` - SVG generation and file I/O errors
- Phase validation - Ensures commands are used in correct phase
- Resource validation - Checks for missing images and fonts

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for new functionality
5. Run tests (`cargo test`)
6. Commit changes (`git commit -m 'Add amazing feature'`)
7. Push to branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## License

This project is licensed under the MIT License.

## Acknowledgments

- Built with Rust for performance and safety
- Board description format and SVG layout from [pinoutleaf](https://github.com/splitbrain/pinoutleaf) by Andreas Gohr
- Uses the `svg` crate for vector graphics generation
- CSV parsing powered by the `csv` crate
- Image processing via the `image` crate