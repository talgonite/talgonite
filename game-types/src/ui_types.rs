#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotPanelType {
    #[default]
    None,
    Item,
    Skill,
    Spell,
    Hotbar,
    World,
}
