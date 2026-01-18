# Input System

This module provides a flexible input abstraction that supports both keyboard and gamepad inputs with modifier keys.

## Architecture

### Core Components

1. **`GameAction`** - Logical game actions (e.g., `MoveUp`, `Refresh`)
2. **`KeyBinding`** - Keyboard key + modifiers (Ctrl/Shift/Alt)
3. **`GamepadInputType`** - Gamepad buttons, D-pad, and analog sticks
4. **`InputSource`** - Unified enum for keyboard or gamepad input
5. **`UnifiedInputBindings`** - Maps actions to multiple input sources

### Usage

#### Keyboard Only (Current)

```rust
fn my_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<InputBindings>,
) {
    if bindings.is_just_pressed(GameAction::Refresh, &keyboard) {
        // Handle refresh
    }
}
```

#### Keyboard + Gamepad (Unified)

```rust
fn my_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad_buttons: Res<ButtonInput<GamepadButton>>,
    gamepad_axes: Res<Axis<GamepadAxis>>,
    unified: Res<UnifiedInputBindings>,
    gamepad_settings: Res<GamepadSettings>,
) {
    if unified.is_just_pressed(
        GameAction::Refresh,
        &keyboard,
        Some(&gamepad_buttons),
        Some(&gamepad_settings),
    ) {
        // Works with both F5 or Select button
    }
}
```

## Modifier Keys

Keyboard bindings support Ctrl/Shift/Alt modifiers:

```rust
// User presses Ctrl+R
let binding = KeyBinding::from_dom_code("Ctrl+KeyR").unwrap();

// Settings file stores: "Ctrl+F5"
// UI displays: "Ctrl+F5"
```

## Gamepad Support

### Steam Deck Specific

The Steam Deck's gamepad is detected automatically through Bevy's gamepad system. Default bindings:

- **Movement**: D-Pad or Left Stick
- **Inventory**: Y button
- **Skills**: X button
- **Spells**: B button
- **Settings**: Start button
- **Refresh**: Select button

### Analog Stick Configuration

Adjust the stick threshold in `GamepadSettings`:

```rust
gamepad_settings.stick_threshold = 0.3; // Default: 0.5
```

### Multiple Gamepads

The system automatically selects the first connected gamepad. Change it:

```rust
gamepad_settings.primary_gamepad = Some(specific_gamepad);
```

## Default Bindings

Each action can have multiple input sources:

| Action | Keyboard | Gamepad |
|--------|----------|---------|
| Move Up | Arrow Up | D-Pad Up / Left Stick Up |
| Move Down | Arrow Down | D-Pad Down / Left Stick Down |
| Move Left | Arrow Left | D-Pad Left / Left Stick Left |
| Move Right | Arrow Right | D-Pad Right / Left Stick Right |
| Inventory | I | Y/Triangle |
| Skills | K | X/Square |
| Spells | P | B/Circle |
| Settings | Escape | Start |
| Refresh | F5 | Select |

## Migration Guide

### From Direct Settings Access

**Before:**
```rust
let key_refresh = dom_code_to_keycode(&settings.key_bindings.refresh)
    .unwrap_or(KeyCode::F5);
if keyboard.just_pressed(key_refresh) { }
```

**After:**
```rust
if bindings.is_just_pressed(GameAction::Refresh, &keyboard) { }
```

### Adding Gamepad to Existing System

1. Change `InputBindings` → `UnifiedInputBindings`
2. Add gamepad resources to system parameters
3. Pass gamepad inputs to `is_pressed`/`is_just_pressed`

## Extending

### Adding a New Action

1. Add variant to `GameAction` enum
2. Add default binding in `UnifiedInputBindings::with_defaults()`
3. Add to settings UI
4. Add to `KeyBindings` struct in settings

### Custom Gamepad Layout

```rust
unified.add_binding(
    GameAction::Inventory,
    InputSource::Gamepad(GamepadInputType::Button(GamepadButtonType::East))
);
```

## Implementation Notes

- **Keyboard**: Processed through Slint → queued → Bevy ButtonInput
- **Gamepad**: Directly through Bevy's gamepad system (bypasses Slint)
- **Modifiers**: Captured during rebind from Slint's KeyEvent.modifiers
- **Persistence**: Keyboard bindings saved to settings.ron, gamepad uses defaults

## Future Enhancements

- [ ] Gamepad button remapping UI
- [ ] Gyro support (Steam Deck)
- [ ] Haptic feedback
- [ ] Dead zone configuration
- [ ] Simultaneous multi-gamepad support
- [ ] Input chords (sequences)
