use bincode::{Decode, Encode};
use byteorder::{LE, ReadBytesExt};

use crate::epf::AnimationDirection;

#[derive(Encode, Decode, Debug)]
pub struct MpfFile {
    pub palette_number: u8,
    pub width: u16,
    pub height: u16,
    pub animations: Vec<MpfAnimation>,
    pub frames: Vec<MpfFrame>,
}

#[derive(Encode, Decode, PartialEq, Debug, Clone, Copy, Hash, Eq)]
pub enum MpfAnimationType {
    Walk,
    Standing,
    Attack,
    Attack2,
    Attack3,
    Extra(u8),
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct MpfAnimation {
    pub animation_type: MpfAnimationType,
    pub frame_count: u8,
    pub frame_index_towards: u8,
    pub frame_index_away: u8,
}

impl MpfAnimation {
    pub fn new(
        anim_type: MpfAnimationType,
        away_frame_index: u8,
        frame_count: u8,
        can_move: bool,
    ) -> Self {
        Self {
            animation_type: anim_type,
            frame_count,
            frame_index_away: away_frame_index,
            frame_index_towards: if can_move {
                away_frame_index + frame_count
            } else {
                away_frame_index
            },
        }
    }

    pub fn frame_index_for_direction(&self, anim_dir: AnimationDirection) -> u8 {
        match anim_dir {
            AnimationDirection::Towards => self.frame_index_towards,
            AnimationDirection::Away => self.frame_index_away,
        }
    }
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct MpfFrame {
    pub top: i16,
    pub left: i16,
    pub bottom: i16,
    pub right: i16,
    pub center_x: i16,
    pub center_y: i16,
    // pub start_address: i32,
    pub data: Vec<u8>,
}

const SIZE_LIMIT: usize = 0x100000; // 640 KiB

impl MpfFile {
    pub fn read_from_da<R: std::io::Read + std::io::Seek>(reader: &mut R) -> std::io::Result<Self> {
        // If the header type is -1, we have a 12-byte header to skip
        // otherwise there is no header and we can use the 4 bytes we read
        if reader.read_i32::<LE>()? == -1 {
            if reader.read_i32::<LE>()? == 4 {
                reader.read_exact(&mut [0; 8])?;
            }
        } else {
            reader.seek(std::io::SeekFrom::Current(-4))?;
        };

        let frame_count = reader.read_u8()?;
        let pixel_width = reader.read_i16::<LE>()? as usize;
        let pixel_height = reader.read_i16::<LE>()? as usize;

        let data_length = reader.read_i32::<LE>()? as usize;

        let mut walk_animation = MpfAnimation::new(
            MpfAnimationType::Walk,
            reader.read_u8()?,
            reader.read_u8()?,
            true,
        );
        let can_move = walk_animation.frame_count > 1;

        if walk_animation.frame_count == 1 {
            walk_animation.frame_index_towards = walk_animation.frame_index_away;
        }

        let has_multiple_attacks = reader.read_i16::<LE>()? == -1;
        let anims = if has_multiple_attacks {
            let standing_animation = MpfAnimation::new(
                MpfAnimationType::Standing,
                reader.read_u8()?,
                reader.read_u8()?,
                can_move,
            );
            let optional_frame_count = reader.read_u8()?;
            let extra_anim = MpfAnimation::new(
                MpfAnimationType::Extra(reader.read_u8()?),
                standing_animation.frame_index_away,
                optional_frame_count,
                can_move,
            );

            vec![
                walk_animation,
                standing_animation,
                MpfAnimation::new(
                    MpfAnimationType::Attack,
                    reader.read_u8()?,
                    reader.read_u8()?,
                    can_move,
                ),
                MpfAnimation::new(
                    MpfAnimationType::Attack2,
                    reader.read_u8()?,
                    reader.read_u8()?,
                    can_move,
                ),
                MpfAnimation::new(
                    MpfAnimationType::Attack3,
                    reader.read_u8()?,
                    reader.read_u8()?,
                    can_move,
                ),
                extra_anim,
            ]
        } else {
            reader.seek_relative(-2)?;

            let attack_animation = MpfAnimation::new(
                MpfAnimationType::Attack,
                reader.read_u8()?,
                reader.read_u8()?,
                can_move,
            );
            let standing_animation = MpfAnimation::new(
                MpfAnimationType::Standing,
                reader.read_u8()?,
                reader.read_u8()?,
                can_move,
            );

            let optional_frame_count = reader.read_u8()?;

            let extra_anim = MpfAnimation::new(
                MpfAnimationType::Extra(reader.read_u8()?),
                standing_animation.frame_index_away,
                optional_frame_count,
                can_move,
            );

            vec![
                walk_animation,
                standing_animation,
                attack_animation,
                extra_anim,
            ]
        };

        let mut frames = Vec::with_capacity(frame_count as usize);

        let mut palette_number = 0;

        let mut i = 0;

        while i < frame_count {
            let left = reader.read_i16::<LE>()?;
            let top = reader.read_i16::<LE>()?;
            let right = reader.read_i16::<LE>()?;
            let bottom = reader.read_i16::<LE>()?;
            let center_x = reader.read_i16::<LE>()?;
            let center_y = reader.read_i16::<LE>()?;
            let start_address = reader.read_i32::<LE>()?;

            if left == -1 && top == -1 {
                palette_number = start_address as u8;

                i += 2;
                continue;
            }

            let frame_width = (right - left) as usize;
            let frame_height = (bottom - top) as usize;

            let pixel_size = frame_width * frame_height;

            if pixel_size > SIZE_LIMIT {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Frame size exceeds limit: {} > {} ({} {} {} {})",
                        pixel_size, SIZE_LIMIT, left, top, right, bottom
                    ),
                ));
            }

            frames.push((
                MpfFrame {
                    top,
                    left,
                    bottom,
                    right,
                    center_x,
                    center_y,
                    data: vec![0; frame_width * frame_height],
                },
                start_address,
            ));

            i += 1;
        }

        let data_start = reader.seek(std::io::SeekFrom::End(-(data_length as i64)))?;

        for (frame, start_address) in &mut frames {
            reader.seek(std::io::SeekFrom::Start(data_start + *start_address as u64))?;
            reader.read_exact(&mut frame.data)?;
        }

        let mut final_anims: Vec<MpfAnimation> = anims
            .iter()
            .filter(|a| a.frame_count > 0)
            .cloned()
            .collect();

        if !final_anims
            .iter()
            .any(|a| a.animation_type == MpfAnimationType::Standing)
        {
            if let Some(walk_anim) = final_anims
                .iter()
                .find(|a| a.animation_type == MpfAnimationType::Walk)
            {
                final_anims.push(MpfAnimation {
                    animation_type: MpfAnimationType::Standing,
                    frame_count: 1,
                    frame_index_away: walk_anim.frame_index_away,
                    frame_index_towards: walk_anim.frame_index_towards,
                })
            }
        }

        Ok(Self {
            palette_number,
            width: pixel_width as u16,
            height: pixel_height as u16,
            animations: final_anims,
            frames: frames.into_iter().map(|(frame, _)| frame).collect(),
        })
    }
}
