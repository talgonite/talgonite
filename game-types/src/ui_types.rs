#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotPanelType {
    #[default]
    None,
    Item,
    Gold,
    Skill,
    Spell,
    Hotbar,
    World,
}
