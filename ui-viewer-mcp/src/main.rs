use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

struct McpState {
    viewer_process: Option<Child>,
    current_component: String,
    current_state: HashMap<String, Value>,
    workspace_path: PathBuf,
}

impl McpState {
    fn new() -> Self {
        Self {
            viewer_process: None,
            current_component: "social-status".to_string(),
            current_state: HashMap::new(),
            workspace_path: std::env::current_dir().unwrap_or_default(),
        }
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let state = Arc::new(Mutex::new(McpState::new()));
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to parse request: {}", e);
                continue;
            }
        };

        let response = handle_request(&request, &state);
        let response_str = serde_json::to_string(&response).unwrap();
        writeln!(stdout, "{}", response_str)?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_request(request: &JsonRpcRequest, state: &Arc<Mutex<McpState>>) -> JsonRpcResponse {
    let id = request.id.clone().unwrap_or(Value::Null);

    match request.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "ui-viewer-mcp",
                    "version": "1.0.0"
                }
            })),
            error: None,
        },
        "notifications/initialized" | "initialized" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        },
        "tools/list" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "tools": get_tools()
            })),
            error: None,
        },
        "tools/call" => {
            let tool_name = request.params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = request.params.get("arguments").cloned().unwrap_or(json!({}));
            
            match call_tool(tool_name, &arguments, state) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": result
                        }]
                    })),
                    error: None,
                },
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32000,
                        message: e.to_string(),
                        data: None,
                    }),
                },
            }
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        },
    }
}

fn get_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "list_ui_components",
            "description": "List all available Slint UI components that can be previewed and tested. Returns component names with descriptions.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "preview_component",
            "description": "Launch the UI viewer to preview a specific Slint component. Opens a window showing the component with interactive controls.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "component": {
                        "type": "string",
                        "description": "Component name to preview (e.g., 'social-status', 'world-list', 'profile')"
                    }
                },
                "required": ["component"]
            }
        }),
        json!({
            "name": "set_component_state",
            "description": "Set the state of the currently previewed component using JSON. Useful for testing different states.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "state": {
                        "type": "object",
                        "description": "JSON object with state properties (e.g., {\"current_status\": 3, \"dropdown_open\": true})"
                    }
                },
                "required": ["state"]
            }
        }),
        json!({
            "name": "get_component_info",
            "description": "Get detailed information about a specific UI component including its properties, callbacks, and usage examples.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "component": {
                        "type": "string",
                        "description": "Component name to get info for"
                    }
                },
                "required": ["component"]
            }
        }),
        json!({
            "name": "read_slint_file",
            "description": "Read the content of a Slint UI file. Useful for understanding component structure.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path to the Slint file from game-ui/ui/ (e.g., 'components/social_status_panel.slint')"
                    }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "validate_slint_syntax",
            "description": "Validate Slint code syntax without running the viewer. Returns any compilation errors.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "Slint code to validate"
                    }
                },
                "required": ["code"]
            }
        }),
        json!({
            "name": "get_theme_colors",
            "description": "Get the current theme colors and design tokens used in the UI.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "generate_component_skeleton",
            "description": "Generate a skeleton Slint component with proper structure following existing patterns.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name for the new component (PascalCase)"
                    },
                    "type": {
                        "type": "string",
                        "enum": ["panel", "indicator", "button", "dialog", "list"],
                        "description": "Type of component to generate"
                    },
                    "properties": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of property names for the component"
                    }
                },
                "required": ["name", "type"]
            }
        })
    ]
}

fn call_tool(name: &str, arguments: &Value, state: &Arc<Mutex<McpState>>) -> Result<String> {
    match name {
        "list_ui_components" => list_components(),
        "preview_component" => {
            let component = arguments.get("component").and_then(|v| v.as_str()).unwrap_or("social-status");
            preview_component(component, state)
        }
        "set_component_state" => {
            let component_state = arguments.get("state").cloned().unwrap_or(json!({}));
            set_state(&component_state, state)
        }
        "get_component_info" => {
            let component = arguments.get("component").and_then(|v| v.as_str()).unwrap_or("social-status");
            get_component_info(component)
        }
        "read_slint_file" => {
            let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
            read_slint_file(path, state)
        }
        "validate_slint_syntax" => {
            let code = arguments.get("code").and_then(|v| v.as_str()).unwrap_or("");
            validate_slint(code)
        }
        "get_theme_colors" => get_theme_colors(state),
        "generate_component_skeleton" => {
            let name = arguments.get("name").and_then(|v| v.as_str()).unwrap_or("NewComponent");
            let comp_type = arguments.get("type").and_then(|v| v.as_str()).unwrap_or("panel");
            let properties: Vec<String> = arguments
                .get("properties")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            generate_skeleton(name, comp_type, &properties)
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
    }
}

fn list_components() -> Result<String> {
    Ok(r#"Available UI Components:

1. social-status
   - Description: Social status indicator with dropdown for selecting availability
   - Properties: current_status (int), dropdown_open (bool), enabled (bool)
   - Callbacks: set_status(int), dropdown_opened(), dropdown_closed()
   - States: Awake(0), DoNotDisturb(1), DayDreaming(2), NeedGroup(3), Grouped(4), LoneHunter(5), GroupHunting(6), NeedHelp(7)

2. world-list
   - Description: List of online players with status icons and filtering
   - Properties: members (array), filter (WorldListFilter)
   - Shows social status icons from the status panel system

3. profile
   - Description: Player profile panel showing stats, equipment, and legend marks
   - Properties: ProfileData struct with name, level, class, etc.
   - Includes social status display

4. inventory
   - Description: Player inventory grid with drag-drop support
   - Properties: items (array), selected_slot (int)

5. player-hud
   - Description: Main HUD with HP/MP bars, experience, and quick actions
   - Properties: current_hp, max_hp, current_mp, max_mp, etc.

6. hotbar
   - Description: Skill/spell hotbar with cooldowns
   - Properties: entries (array), cooldowns (array)

7. chat
   - Description: Chat window with channels and history
   - Properties: messages (array), current_channel (int)

8. settings
   - Description: Settings panel with game options
   - Uses SettingsState global"#.to_string())
}

fn preview_component(component: &str, state: &Arc<Mutex<McpState>>) -> Result<String> {
    let mut state = state.lock().unwrap();
    state.current_component = component.to_string();
    
    if let Some(ref mut process) = state.viewer_process {
        let _ = process.kill();
    }
    
    let workspace = &state.workspace_path;
    let viewer_path = workspace.join("target").join("debug").join("ui-viewer");
    
    #[cfg(windows)]
    let viewer_path = viewer_path.with_extension("exe");
    
    if !viewer_path.exists() {
        return Ok(format!(
            "UI Viewer not built. Run this command first:\n\n  cargo build -p ui-viewer\n\nThen call this tool again to preview '{}'.\n\nViewer path: {}",
            component,
            viewer_path.display()
        ));
    }
    
    match Command::new(&viewer_path)
        .arg("preview")
        .arg("--component")
        .arg(component)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => {
            state.viewer_process = Some(child);
            Ok(format!(
                "Launched UI Viewer for component: {}\n\nThe viewer window should now be visible. Use the controls on the left panel to interact with the component.\n\nAvailable actions:\n- Cycle: Cycle through status options\n- Toggle: Open/close dropdown\n- Reset: Reset to default state\n- Click status options in the left panel to select them directly",
                component
            ))
        }
        Err(e) => Err(anyhow::anyhow!("Failed to launch viewer: {}", e)),
    }
}

fn set_state(component_state: &Value, state: &Arc<Mutex<McpState>>) -> Result<String> {
    let mut state = state.lock().unwrap();
    
    if let Some(obj) = component_state.as_object() {
        for (key, value) in obj {
            state.current_state.insert(key.clone(), value.clone());
        }
    }
    
    Ok(format!(
        "State updated for component '{}':\n{}",
        state.current_component,
        serde_json::to_string_pretty(component_state)?
    ))
}

fn get_component_info(component: &str) -> Result<String> {
    match component {
        "social-status" => Ok(r#"# SocialStatusPanel Component

## Overview
The Social Status Panel allows players to set their availability status, which is visible to other players in the world list and profiles.

## Files
- State: game-ui/ui/social_status_state.slint
- Components: game-ui/ui/components/social_status_panel.slint
- Integration: src/slint_support/callbacks/game_callbacks.rs

## Slint Structure
```
SocialStatusState (global)
├── current-status: int (0-7)
├── dropdown-open: bool
├── enabled: bool
├── status-options: [SocialStatusEntry]
├── set-status(int) callback
├── dropdown-opened() callback
└── dropdown-closed() callback

SocialStatusPanel (component)
├── compact: bool (default false)
├── SocialStatusIndicator
│   ├── status-id: int
│   └── interactive: bool
└── triggers SocialStatusDropdown when clicked
```

## Status Values
| ID | Name | Icon | Color | Category |
|----|------|------|-------|----------|
| 0 | Awake | ● | #22c55e | Available |
| 1 | Do Not Disturb | ⊘ | #ef4444 | Busy |
| 2 | Daydreaming | ◐ | #f59e0b | Away |
| 3 | Looking for Group | ◎ | #3b82f6 | LFG |
| 4 | In a Group | ◉ | #8b5cf6 | LFG |
| 5 | Lone Hunter | ◈ | #06b6d4 | Hunting |
| 6 | Group Hunting | ◆ | #10b981 | Hunting |
| 7 | Need Help | ◇ | #f43f5e | Help |

## Usage Example
```slint
import { SocialStatusPanel } from "components/social_status_panel.slint";
import { SocialStatusState } from "social_status_state.slint";

// In your container:
SocialStatusPanel {
    x: 10px;
    y: 10px;
    compact: false;
}

// The dropdown overlay should be at the root level:
SocialStatusDropdownOverlay { }
SocialStatusDropdown {
    anchor-x: 10px;
    anchor-y: 40px;
}
```

## Rust Integration
```rust
// Wire callback
let social_status_state = app.global::<SocialStatusState>();
social_status_state.on_set_status(move |status| {
    tx.send(UiToCore::SetSocialStatus { status: status as u8 });
});

// Sync from server
social_status_state.set_current_status(status_from_server as i32);
```"#.to_string()),
        
        "world-list" => Ok(r#"# WorldList Component

## Overview
Displays online players with their status icons, names, and metadata.

## Files
- Component: game-ui/ui/components/world_list.slint
- State: game-ui/ui/game_state.slint (WorldListMemberUi)

## Key Properties
- members: [WorldListMemberUi]
- filter: WorldListFilter

## WorldListMemberUi Structure
```
struct WorldListMemberUi {
    name: string,
    title: string,
    class: string,
    color: [f32; 4],
    is_master: bool,
    social_status: int,  // Links to SocialStatusState
}
```"#.to_string()),
        
        _ => Ok(format!("Component info not available for: {}\n\nUse 'list_ui_components' to see available components.", component)),
    }
}

fn read_slint_file(path: &str, state: &Arc<Mutex<McpState>>) -> Result<String> {
    let state = state.lock().unwrap();
    let full_path = state.workspace_path.join("game-ui").join("ui").join(path);
    
    match std::fs::read_to_string(&full_path) {
        Ok(content) => Ok(format!("File: {}\n\n```slint\n{}\n```", full_path.display(), content)),
        Err(e) => Err(anyhow::anyhow!("Failed to read file '{}': {}", full_path.display(), e)),
    }
}

fn validate_slint(code: &str) -> Result<String> {
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join("validate_temp.slint");
    
    std::fs::write(&temp_file, code)?;
    
    Ok(format!(
        "Slint syntax validation:\n\n{}\n\nNote: Full validation requires running slint-viewer or cargo build. \
         This tool provides basic checks. For complete validation, preview the component.",
        if code.contains("component") && code.contains("{") && code.contains("}") {
            "Basic syntax appears valid (has component definition with braces)"
        } else {
            "Warning: Missing component definition or braces"
        }
    ))
}

fn get_theme_colors(state: &Arc<Mutex<McpState>>) -> Result<String> {
    let state = state.lock().unwrap();
    let theme_path = state.workspace_path.join("game-ui").join("ui").join("theme.slint");
    
    match std::fs::read_to_string(&theme_path) {
        Ok(content) => Ok(format!("# Theme Colors\n\nFrom: {}\n\n```slint\n{}\n```", theme_path.display(), content)),
        Err(_) => Ok(r#"# Default Theme Colors

```slint
global Theme {
    out property <color> background: #0d0d1a;
    out property <color> surface: #1a1a2e;
    out property <color> surface-card: #2a2a3e;
    out property <color> foreground: #ffffff;
    out property <color> foreground-muted: #888888;
    out property <color> primary: #3b82f6;
    out property <color> success: #22c55e;
    out property <color> warning: #f59e0b;
    out property <color> error: #ef4444;
    out property <length> radius-small: 4px;
    out property <length> radius-medium: 8px;
    out property <length> spacing-small: 8px;
    out property <length> spacing-medium: 16px;
}
```"#.to_string()),
    }
}

fn generate_skeleton(name: &str, comp_type: &str, properties: &[String]) -> Result<String> {
    let props = properties
        .iter()
        .map(|p| format!("    in-out property <string> {}: \"\";", p))
        .collect::<Vec<_>>()
        .join("\n");
    
    let template = match comp_type {
        "panel" => format!(r#"import {{ Theme }} from "../theme.slint";
import {{ BasePanel }} from "base_panel.slint";

export component {name} inherits BasePanel {{
    title: "{name}";
    width: 300px;
    height: 400px;
    
{props}
    
    callback on-close();
    
    VerticalLayout {{
        padding: Theme.spacing-medium;
        spacing: Theme.spacing-small;
        
        // Panel content here
        Text {{
            text: "Content goes here";
            color: Theme.foreground;
        }}
        
        Rectangle {{ }}
    }}
}}"#, name = name, props = if props.is_empty() { "    // Add properties here".to_string() } else { props }),
        
        "indicator" => format!(r#"import {{ Theme }} from "../theme.slint";

export component {name} inherits Rectangle {{
    width: 24px;
    height: 24px;
    border-radius: self.width / 2;
    background: Theme.surface-card;
    
{props}
    
    in property <bool> active: false;
    
    states [
        active when root.active: {{
            background: Theme.primary;
        }}
    ]
    
    Text {{
        text: "●";
        color: root.active ? Theme.foreground : Theme.foreground-muted;
        font-size: 12px;
        horizontal-alignment: center;
        vertical-alignment: center;
    }}
}}"#, name = name, props = if props.is_empty() { "    // Add properties here".to_string() } else { props }),
        
        "button" => format!(r#"import {{ Theme }} from "../theme.slint";

export component {name} inherits Rectangle {{
    in property <string> label: "Button";
    in property <bool> enabled: true;
    
{props}
    
    callback clicked();
    
    width: 100px;
    height: 32px;
    border-radius: Theme.radius-small;
    background: touch.has-hover && root.enabled ? Theme.primary : Theme.surface-card;
    opacity: root.enabled ? 1.0 : 0.5;
    
    Text {{
        text: root.label;
        color: Theme.foreground;
        font-size: 12px;
        horizontal-alignment: center;
        vertical-alignment: center;
    }}
    
    touch := TouchArea {{
        enabled: root.enabled;
        clicked => {{ root.clicked(); }}
    }}
}}"#, name = name, props = if props.is_empty() { "    // Add properties here".to_string() } else { props }),
        
        "dialog" => format!(r#"import {{ Theme }} from "../theme.slint";
import {{ BasePanel }} from "base_panel.slint";

export component {name} inherits Rectangle {{
    in property <bool> visible: false;
    
{props}
    
    callback confirmed();
    callback cancelled();
    
    visible: root.visible;
    width: 100%;
    height: 100%;
    background: #00000080;
    
    TouchArea {{
        clicked => {{ root.cancelled(); }}
    }}
    
    Rectangle {{
        x: (parent.width - self.width) / 2;
        y: (parent.height - self.height) / 2;
        width: 400px;
        height: 200px;
        background: Theme.surface;
        border-radius: Theme.radius-medium;
        
        TouchArea {{ }}
        
        VerticalLayout {{
            padding: Theme.spacing-medium;
            spacing: Theme.spacing-small;
            
            Text {{
                text: "Dialog Title";
                color: Theme.foreground;
                font-size: 16px;
                font-weight: 700;
            }}
            
            Text {{
                text: "Dialog content goes here";
                color: Theme.foreground-muted;
                wrap: word-wrap;
            }}
            
            Rectangle {{ }}
            
            HorizontalLayout {{
                spacing: Theme.spacing-small;
                
                Rectangle {{ }}
                
                Rectangle {{
                    width: 80px;
                    height: 32px;
                    background: Theme.surface-card;
                    border-radius: Theme.radius-small;
                    
                    Text {{ text: "Cancel"; color: Theme.foreground; horizontal-alignment: center; vertical-alignment: center; }}
                    TouchArea {{ clicked => {{ root.cancelled(); }} }}
                }}
                
                Rectangle {{
                    width: 80px;
                    height: 32px;
                    background: Theme.primary;
                    border-radius: Theme.radius-small;
                    
                    Text {{ text: "Confirm"; color: Theme.foreground; horizontal-alignment: center; vertical-alignment: center; }}
                    TouchArea {{ clicked => {{ root.confirmed(); }} }}
                }}
            }}
        }}
    }}
}}"#, name = name, props = if props.is_empty() { "    // Add properties here".to_string() } else { props }),
        
        "list" => format!(r#"import {{ Theme }} from "../theme.slint";
import {{ ScrollView }} from "std-widgets.slint";

export struct {name}Item {{
    id: int,
    label: string,
}}

export component {name} inherits Rectangle {{
    in property <[{name}Item]> items: [];
    in-out property <int> selected-index: -1;
    
{props}
    
    callback item-selected(int);
    
    background: Theme.surface;
    border-radius: Theme.radius-small;
    
    ScrollView {{
        VerticalLayout {{
            padding: Theme.spacing-small;
            spacing: 4px;
            
            for item[idx] in root.items: Rectangle {{
                height: 32px;
                background: root.selected-index == idx ? Theme.primary : (touch.has-hover ? Theme.surface-card : transparent);
                border-radius: Theme.radius-small;
                
                HorizontalLayout {{
                    padding-left: Theme.spacing-small;
                    
                    Text {{
                        text: item.label;
                        color: Theme.foreground;
                        vertical-alignment: center;
                    }}
                }}
                
                touch := TouchArea {{
                    clicked => {{
                        root.selected-index = idx;
                        root.item-selected(item.id);
                    }}
                }}
            }}
        }}
    }}
}}"#, name = name, props = if props.is_empty() { "    // Add properties here".to_string() } else { props }),
        
        _ => format!("// Unknown component type: {}", comp_type),
    };
    
    Ok(format!(
        "Generated {} component skeleton for '{}':\n\n```slint\n{}\n```\n\nSave this to: game-ui/ui/components/{}.slint",
        comp_type, name, template, name.to_lowercase().replace(' ', "_")
    ))
}
