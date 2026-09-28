# Changelog

All notable changes to this crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-09-28

### Added

- `model::Pinout` describes a board in the YAML format of
  [pinoutleaf](https://github.com/splitbrain/pinoutleaf): up to four rows of
  pins whose labels are written as `label:type`, custom types, board photos
  and row offsets. It reads and writes YAML and JSON, and existing
  pinoutleaf files load as they are. Width and height default to the
  longest rows, and a `notes` extension holds free text such as feature
  lists.
- `renderer::leaf::render_svg` draws a board as SVG in pinoutleaf's layout,
  true to scale in millimeters, with embedded board photos, a legend and a
  mirrored back view.
- `renderer::term::render_terminal` draws a board as ANSI truecolor text:
  label chips in their type colors aligned in columns, a board with a pad
  per pin, top and bottom rows and a legend. Long labels are shortened from
  the middle (`GPIO36` to `GP…36`) until the diagram fits the width.
  `render_lines`, `label_chip` and `legend_entries` give the same drawing as
  styled spans for other terminal libraries.
- `import::from_csv_file` converts the legacy CSV descriptions to the
  model, guessing label types from datasheet naming.
- The `pinout` command line tool, built with the default `cli` feature. It
  reads YAML, JSON or legacy CSV files and writes the terminal diagram, SVG,
  JSON or YAML.
- `pinout --tui` opens an interactive terminal UI: a YAML editor with syntax
  highlighting and vim keys, front and back views that redraw on every
  edit, a pins table, the board's notes and a chooser for the label columns
  to show.
- `ATtiny85.yaml` and `ESP32-MAXIO.yaml` examples.

### Removed

- The placeholder `add` function.

## [0.1.0] - 2025-09-29

### Added

- CSV pinout descriptions rendered to SVG, in the format of
  [GenPinoutSVG](https://github.com/stevenj/GenPinoutSVG).

[Unreleased]: https://github.com/orhanbalci/pinout/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/orhanbalci/pinout/releases/tag/v0.2.0
[0.1.0]: https://crates.io/crates/pinout/0.1.0
