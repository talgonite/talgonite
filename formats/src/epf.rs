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
    // Emote only - only in emot01-04.epf
    Smile,          // frame 1 in emot01.epf
    Cry,            // frame 2 in emot01.epf
    Sad,            // frame 3 in emot01.epf
    Wink,           // frame 4 in emot01.epf
    Stunned,        // frame 5 in emot01.epf
    Raz,            // frame 6 in emot01.epf
    Surprise,       // frame 7 in emot01.epf
    Sleepy,         // frames 8+9 in emot01.epf
    Yawn,           // frames 10+11 in emot01.epf
    BalloonElder,   // frame 25 in emot01.epf
    BalloonJoy,     // frame 26 in emot01.epf
    BalloonSlick,   // frame 27 in emot01.epf
    BalloonScheme,  // frame 28 in emot01.epf
    BalloonLaser,   // frame 29 in emot01.epf
    BalloonGloom,   // frame 30 in emot01.epf
    BalloonAwe,     // frame 31 in emot01.epf
    BalloonShadow,  // frame 32 in emot01.epf
    BalloonSob,     // frames 33-35 in emot01.epf
    BalloonFire,    // frames 36-38 in emot01.epf
    BalloonDizzy,   // frames 39-42 in emot01.epf
    SymbolRock,     // frame 12 in emot01.epf
    SymbolScissors, // frame 13 in emot01.epf
    SymbolPaper,    // frame 14 in emot01.epf
    SymbolScramble, // frame 15 in emot01.epf
    SymbolSilence,  // frames 16-18 in emot01.epf
    Mask,           // frame 19 in emot01.epf
    Blush,          // frame 20 in emot01.epf
    SymbolLove,     // frame 21 in emot01.epf
    SymbolSweat,    // frame 22 in emot01.epf
    SymbolMusic,    // frame 23 in emot01.epf
    SymbolAngry,    // frame 24 in emot01.epf
}

impl EpfAnimationType {
    pub fn is_emote(&self) -> bool {
        matches!(
            self,
            EpfAnimationType::Smile
                | EpfAnimationType::Cry
                | EpfAnimationType::Sad
                | EpfAnimationType::Wink
                | EpfAnimationType::Stunned
                | EpfAnimationType::Raz
                | EpfAnimationType::Surprise
                | EpfAnimationType::Sleepy
                | EpfAnimationType::Yawn
                | EpfAnimationType::BalloonElder
                | EpfAnimationType::BalloonJoy
                | EpfAnimationType::BalloonSlick
                | EpfAnimationType::BalloonScheme
                | EpfAnimationType::BalloonLaser
                | EpfAnimationType::BalloonGloom
                | EpfAnimationType::BalloonAwe
                | EpfAnimationType::BalloonShadow
                | EpfAnimationType::BalloonSob
                | EpfAnimationType::BalloonFire
                | EpfAnimationType::BalloonDizzy
                | EpfAnimationType::SymbolRock
                | EpfAnimationType::SymbolPaper
                | EpfAnimationType::SymbolScissors
                | EpfAnimationType::SymbolScramble
                | EpfAnimationType::SymbolSilence
                | EpfAnimationType::Mask
                | EpfAnimationType::Blush
                | EpfAnimationType::SymbolLove
                | EpfAnimationType::SymbolSweat
                | EpfAnimationType::SymbolMusic
                | EpfAnimationType::SymbolAngry
        )
    }
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
            "emot" => vec![
                (EpfAnimationType::Smile, AnimationDirection::Towards, 0..1),
                (EpfAnimationType::Cry, AnimationDirection::Towards, 1..2),
                (EpfAnimationType::Sad, AnimationDirection::Towards, 2..3),
                (EpfAnimationType::Wink, AnimationDirection::Towards, 3..4),
                (EpfAnimationType::Stunned, AnimationDirection::Towards, 4..5),
                (EpfAnimationType::Raz, AnimationDirection::Towards, 5..6),
                (
                    EpfAnimationType::Surprise,
                    AnimationDirection::Towards,
                    6..7,
                ),
                (EpfAnimationType::Sleepy, AnimationDirection::Towards, 7..9),
                (EpfAnimationType::Yawn, AnimationDirection::Towards, 9..11),
                (
                    EpfAnimationType::SymbolRock,
                    AnimationDirection::Towards,
                    11..12,
                ),
                (
                    EpfAnimationType::SymbolScissors,
                    AnimationDirection::Towards,
                    12..13,
                ),
                (
                    EpfAnimationType::SymbolPaper,
                    AnimationDirection::Towards,
                    13..14,
                ),
                (
                    EpfAnimationType::SymbolScramble,
                    AnimationDirection::Towards,
                    14..15,
                ),
                (
                    EpfAnimationType::SymbolSilence,
                    AnimationDirection::Towards,
                    15..18,
                ),
                (EpfAnimationType::Mask, AnimationDirection::Towards, 18..19),
                (EpfAnimationType::Blush, AnimationDirection::Towards, 19..20),
                (
                    EpfAnimationType::SymbolLove,
                    AnimationDirection::Towards,
                    20..21,
                ),
                (
                    EpfAnimationType::SymbolSweat,
                    AnimationDirection::Towards,
                    21..22,
                ),
                (
                    EpfAnimationType::SymbolMusic,
                    AnimationDirection::Towards,
                    22..23,
                ),
                (
                    EpfAnimationType::SymbolAngry,
                    AnimationDirection::Towards,
                    23..24,
                ),
                (
                    EpfAnimationType::BalloonElder,
                    AnimationDirection::Towards,
                    24..25,
                ),
                (
                    EpfAnimationType::BalloonJoy,
                    AnimationDirection::Towards,
                    25..26,
                ),
                (
                    EpfAnimationType::BalloonSlick,
                    AnimationDirection::Towards,
                    26..27,
                ),
                (
                    EpfAnimationType::BalloonScheme,
                    AnimationDirection::Towards,
                    27..28,
                ),
                (
                    EpfAnimationType::BalloonLaser,
                    AnimationDirection::Towards,
                    28..29,
                ),
                (
                    EpfAnimationType::BalloonGloom,
                    AnimationDirection::Towards,
                    29..30,
                ),
                (
                    EpfAnimationType::BalloonAwe,
                    AnimationDirection::Towards,
                    30..31,
                ),
                (
                    EpfAnimationType::BalloonShadow,
                    AnimationDirection::Towards,
                    31..32,
                ),
                (
                    EpfAnimationType::BalloonSob,
                    AnimationDirection::Towards,
                    32..35,
                ),
                (
                    EpfAnimationType::BalloonFire,
                    AnimationDirection::Towards,
                    35..38,
                ),
                (
                    EpfAnimationType::BalloonDizzy,
                    AnimationDirection::Towards,
                    38..42,
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
