# Pinout

A Rust library and command-line tool for generating beautiful pinout diagrams in SVG format from CSV descriptions. This tool is designed specifically for creating graphical pinout datasheets for microcontrollers, development boards, and electronic components.

## Features

- **CSV-based Configuration**: Define pinout diagrams using simple CSV files with a structured command format
- **Two-Phase Rendering**: Setup phase for themes and styling, Draw phase for actual rendering
- **Rich Theming System**: Customizable colors, fonts, borders, and styling for different pin types and groups
- **SVG Output**: Vector-based output for crisp, scalable diagrams
- **Pin Grouping**: Organize pins by type (IO, Input, Output) or custom groups with individual styling
- **Image and Icon Support**: Embed images and SVG icons in your diagrams
- **Flexible Layout**: Support for different page sizes (A3, A4) and orientations
- **Font Customization**: Support for custom fonts, sizes, colors, and styles

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

1. **Create a simple CSV file** (`my_pinout.csv`):
```csv
LABELS,DEFAULT,TYPE,GROUP,Pin,Function
BORDER COLOR,black
FILL COLOR,white,lightblue,yellow
FONT,Arial
FONT SIZE,10

TYPE,IO,blue,1
TYPE,Input,green,1
TYPE,Output,red,1
GROUP,IO,lightblue,0.5
GROUP,Input,lightgreen,0.5
GROUP,Output,lightyellow,0.5

PAGE,A4-L
DPI,150

DRAW
ANCHOR,50,100
PINSET,LEFT,PACKED,CENTER,CENTER,25,60,80,10,5,2
PIN,1,VCC,Output,Output,3.3V Power
PIN,2,GND,Output,Output,Ground
PIN,3,D2,IO,IO,Digital Pin
PIN,4,A0,Input,Input,Analog Input
```

2. **Generate the SVG**:
```bash
cargo run --example main my_pinout.csv my_pinout.svg
```

3. **View the result**: Open `my_pinout.svg` in any web browser or vector graphics editor.

## Usage

### Command Line Tool

Generate an SVG pinout diagram from a CSV file:

```bash
cargo run --example main input.csv output.svg
```

Options:
- `--overwrite` / `-o`: Overwrite existing SVG files
- `--help` / `-h`: Show help information

If no output file is specified, the tool will create an SVG file with the same name as the input CSV file.

### As a Library

```rust
use pinout::parser::csv::parse_csv_file;
use pinout::renderer::svg::generate_svg;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CSV file into commands
    let commands = parse_csv_file("pinout.csv")?;
    
    // Generate SVG from commands
    generate_svg(&commands, "output.svg")?;
    
    Ok(())
}
```

## CSV Format

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

### ESP32 Development Board
See `ESP32-MAXIO.csv` for a complete example showing:
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
- Uses the `svg` crate for vector graphics generation
- CSV parsing powered by the `csv` crate
- Image processing via the `image` crate