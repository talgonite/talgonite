# UI Viewer MCP Server

MCP (Model Context Protocol) server for Slint UI component visualization and testing. This server allows Cursor AI to interact with and preview UI components.

## Setup

### Build

```bash
cargo build -p ui-viewer-mcp --release
```

### Configure Cursor

Add to your Cursor MCP settings (Settings > MCP):

```json
{
  "mcpServers": {
    "ui-viewer": {
      "command": "ui-viewer-mcp.exe"
    }
  }
}
```

Or use cargo to run:

```json
{
  "mcpServers": {
    "ui-viewer": {
      "command": "cargo",
      "args": ["run", "--package", "ui-viewer-mcp", "--release"],
      "cwd": "src\\talgonite"
    }
  }
}
```

## Available Tools

### list_ui_components
List all available Slint UI components.

### preview_component
Launch the UI viewer to preview a specific component.

```json
{
  "component": "social-status"
}
```

### set_component_state
Set state of the previewed component.

```json
{
  "state": {
    "current_status": 3,
    "dropdown_open": true
  }
}
```

### get_component_info
Get detailed info about a component including properties, callbacks, and examples.

```json
{
  "component": "social-status"
}
```

### read_slint_file
Read a Slint UI file content.

```json
{
  "path": "components/social_status_panel.slint"
}
```

### validate_slint_syntax
Validate Slint code syntax.

```json
{
  "code": "component Test inherits Rectangle { }"
}
```

### get_theme_colors
Get theme colors and design tokens.

### generate_component_skeleton
Generate a component skeleton following patterns.

```json
{
  "name": "MyPanel",
  "type": "panel",
  "properties": ["title", "value"]
}
```

## Example Usage in Cursor

1. Ask: "List all UI components"
2. Ask: "Show me details about the social-status component"
3. Ask: "Generate a new panel component called PlayerStats with properties hp, mp, experience"
4. Ask: "Preview the social-status component"

## Development

The MCP server communicates over stdio using JSON-RPC 2.0.

To test manually:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | cargo run -p ui-viewer-mcp
```
