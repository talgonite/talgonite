# UI Viewer

A standalone Slint UI component viewer and testing tool for the Talgonite game client.

## Usage

### Build

```bash
cargo build -p ui-viewer --release
```

### Run

```bash
# Preview default component (social-status)
cargo run -p ui-viewer

# List available components
cargo run -p ui-viewer -- list

# Preview specific component
cargo run -p ui-viewer -- preview --component social-status

# Test component with custom state
cargo run -p ui-viewer -- test --component social-status --state '{"current_status": 3}'
```

## Features

- Live preview of Slint UI components
- Interactive controls to test different states
- Cycle through status options
- Toggle dropdowns
- Reset to default state

## Available Components

| Component | Description |
|-----------|-------------|
| social-status | Social status indicator with dropdown |
| world-list | Online player list with status icons |
| profile | Player profile panel |
| inventory | Inventory grid with drag-drop |
| player-hud | HP/MP bars and quick actions |

## Screenshots

The viewer shows:
- Left panel: Interactive controls
- Right panel: Component previews at different sizes and states

## Development

To add new components to the viewer:

1. Import the component in `src/viewer.slint`
2. Add a preview section in the `ViewerWindow`
3. Add mock data setup in `src/main.rs`
