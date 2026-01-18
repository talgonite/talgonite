use bincode::{Decode, Encode};

#[derive(Clone, Encode, Decode)]
pub struct EpfFrame {
    pub top: usize,
    pub left: usize,
    pub bottom: usize,
    pub right: usize,
    pub data: Vec<u8>,
}

#[derive(Clone, Encode, Decode)]
pub struct EpfImage {
    pub width: usize,
    pub height: usize,
    pub frames: Vec<EpfFrame>,
}

#[derive(Clone, Copy, Debug, Encode, Decode, PartialEq, Hash, Eq)]
#[repr(u8)]
pub enum EpfAnimationType {
    Walk,
    Idle,
    Attack,
    SpellChant,        // in EPF b
    BardAttack,        // in EPF b
    PrayerChant,       // in EPF b
    ArmsUpChant,       // in EPF 03
    Wave,              // in EPF 03
    BlowKiss,          // in EPF 03
    ShowOffAccessory,  // in EPF 04 - This is a total guess
    TwoHandedAttack,   //in EPF c
    JumpAttack,        //in EPF c
    SwipeAttack,       // in EPF c
    HeavySwipeAttack,  // in EPF c
    HeavyJumpAttack,   // in EPF c
    KickAttack,        // in EPF d
    PunchAttack,       // in EPF d
    LongKickAttack,    // in EPF d
    StabAttack,        // in EPF e
    DoubleStabAttack,  // in EPF e
    ArrowShot,         // in EPF e
    HeavyArrowShot,    // in EPF e
    FarArrowShot,      // in EPF e
    PrayerSummonChant, // in EPF e
    WizardCast,        // in EPF f
    SummonerCast,      // in EPF f
}

#[derive(Clone, Encode, Decode)]
pub struct EpfAnimation {
    pub animation_type: EpfAnimationType,
    pub direction: AnimationDirection,
    pub image: EpfImage,
}

#[derive(Clone, Debug, Encode, Decode, PartialEq, Eq, Hash, Copy)]
pub enum AnimationDirection {
    Away,
    Towards,
}

impl EpfFrame {
    pub fn new(top: usize, left: usize, bottom: usize, right: usize, data: Vec<u8>) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
            data,
        }
    }
}

impl EpfImage {
    fn subset(&self, range: std::ops::Range<usize>) -> Option<Self> {
        self.frames.get(range).map(|frames| Self {
            width: self.width,
            height: self.height,
            frames: frames.to_vec(),
        })
    }

    fn create_animations(
        &self,
        specs: Vec<(EpfAnimationType, AnimationDirection, std::ops::Range<usize>)>,
    ) -> Vec<EpfAnimation> {
        specs
            .into_iter()
            .filter_map(|(animation_type, direction, range)| {
                self.subset(range).map(|image| EpfAnimation {
                    animation_type,
                    direction,
                    image,
                })
            })
            .collect()
    }

    pub fn into_animation(&self, suffix: &str) -> Vec<EpfAnimation> {
        let meh = match suffix {
            "01" => vec![
                (EpfAnimationType::Idle, AnimationDirection::Away, 0..1),
                (EpfAnimationType::Idle, AnimationDirection::Towards, 5..6),
                (EpfAnimationType::Walk, AnimationDirection::Away, 0..5),
                (EpfAnimationType::Walk, AnimationDirection::Towards, 5..10),
            ],
            "02" => vec![
                (EpfAnimationType::Attack, AnimationDirection::Away, 0..2),
                (EpfAnimationType::Attack, AnimationDirection::Towards, 2..4),
            ],
            "03" => vec![
                (
                    EpfAnimationType::ArmsUpChant,
                    AnimationDirection::Away,
                    0..1,
                ),
                (
                    EpfAnimationType::ArmsUpChant,
                    AnimationDirection::Towards,
                    1..2,
                ),
                (EpfAnimationType::BlowKiss, AnimationDirection::Away, 2..4),
                (
                    EpfAnimationType::BlowKiss,
                    AnimationDirection::Towards,
                    4..6,
                ),
                (EpfAnimationType::Wave, AnimationDirection::Away, 6..8),
                (EpfAnimationType::Wave, AnimationDirection::Towards, 8..10),
            ],
            "04" => vec![
                (
                    EpfAnimationType::ShowOffAccessory,
                    AnimationDirection::Away,
                    0..8,
                ),
                (
                    EpfAnimationType::ShowOffAccessory,
                    AnimationDirection::Towards,
                    8..16,
                ),
            ], //TODO: Finalize what this actually is, this is a complete guess
            "b" => vec![
                //priest
                (EpfAnimationType::SpellChant, AnimationDirection::Away, 0..3),
                (
                    EpfAnimationType::SpellChant,
                    AnimationDirection::Towards,
                    3..6,
                ),
                (EpfAnimationType::BardAttack, AnimationDirection::Away, 6..9),
                (
                    EpfAnimationType::BardAttack,
                    AnimationDirection::Towards,
                    9..12,
                ),
                (
                    EpfAnimationType::PrayerChant,
                    AnimationDirection::Away,
                    12..13,
                ),
                (
                    EpfAnimationType::PrayerChant,
                    AnimationDirection::Towards,
                    13..14,
                ),
            ],
            "c" => vec![
                //warrior
                (
                    EpfAnimationType::TwoHandedAttack,
                    AnimationDirection::Away,
                    0..4,
                ),
                (
                    EpfAnimationType::TwoHandedAttack,
                    AnimationDirection::Towards,
                    4..8,
                ),
                (
                    EpfAnimationType::JumpAttack,
                    AnimationDirection::Away,
                    8..11,
                ),
                (
                    EpfAnimationType::JumpAttack,
                    AnimationDirection::Towards,
                    11..14,
                ),
                (
                    EpfAnimationType::SwipeAttack,
                    AnimationDirection::Away,
                    14..16,
                ),
                (
                    EpfAnimationType::SwipeAttack,
                    AnimationDirection::Towards,
                    16..18,
                ),
                (
                    EpfAnimationType::HeavySwipeAttack,
                    AnimationDirection::Away,
                    18..21,
                ),
                (
                    EpfAnimationType::HeavySwipeAttack,
                    AnimationDirection::Towards,
                    21..24,
                ),
                (
                    EpfAnimationType::HeavyJumpAttack,
                    AnimationDirection::Away,
                    24..27,
                ),
                (
                    EpfAnimationType::HeavyJumpAttack,
                    AnimationDirection::Towards,
                    27..30,
                ),
            ],
            "d" => vec![
                //monk
                (EpfAnimationType::KickAttack, AnimationDirection::Away, 0..3),
                (
                    EpfAnimationType::KickAttack,
                    AnimationDirection::Towards,
                    3..6,
                ),
                (
                    EpfAnimationType::PunchAttack,
                    AnimationDirection::Away,
                    6..8,
                ),
                (
                    EpfAnimationType::PunchAttack,
                    AnimationDirection::Towards,
                    8..10,
                ),
                (
                    EpfAnimationType::LongKickAttack,
                    AnimationDirection::Away,
                    10..14,
                ),
                (
                    EpfAnimationType::LongKickAttack,
                    AnimationDirection::Towards,
                    14..18,
                ),
            ],
            "e" => vec![
                //rogue
                (EpfAnimationType::StabAttack, AnimationDirection::Away, 0..2),
                (
                    EpfAnimationType::StabAttack,
                    AnimationDirection::Towards,
                    2..4,
                ),
                (
                    EpfAnimationType::DoubleStabAttack,
                    AnimationDirection::Away,
                    4..6,
                ),
                (
                    EpfAnimationType::DoubleStabAttack,
                    AnimationDirection::Towards,
                    6..8,
                ),
                (EpfAnimationType::ArrowShot, AnimationDirection::Away, 8..12),
                (
                    EpfAnimationType::ArrowShot,
                    AnimationDirection::Towards,
                    12..16,
                ),
                (
                    EpfAnimationType::HeavyArrowShot,
                    AnimationDirection::Away,
                    16..22,
                ),
                (
                    EpfAnimationType::HeavyArrowShot,
                    AnimationDirection::Towards,
                    22..28,
                ),
                (
                    EpfAnimationType::FarArrowShot,
                    AnimationDirection::Away,
                    28..32,
                ),
                (
                    EpfAnimationType::FarArrowShot,
                    AnimationDirection::Towards,
                    32..36,
                ),
            ],
            "f" => vec![
                // wizard
                (EpfAnimationType::WizardCast, AnimationDirection::Away, 0..2),
                (
                    EpfAnimationType::WizardCast,
                    AnimationDirection::Towards,
                    2..4,
                ),
                (
                    EpfAnimationType::SummonerCast,
                    AnimationDirection::Away,
                    4..8,
                ),
                (
                    EpfAnimationType::SummonerCast,
                    AnimationDirection::Towards,
                    8..12,
                ),
            ],
            "05" => vec![], // TODO: Seems like another walk ? For mounts and stuff maybe?
            "0b" => vec![], // TODO: What's in here??
            "1e" => vec![], // TODO: What's in here??
            _ => unreachable!("Unsupported EPF suffix: {}", suffix),
        };
        self.create_animations(meh)
    }
}
