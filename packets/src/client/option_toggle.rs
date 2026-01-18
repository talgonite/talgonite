use num_enum::IntoPrimitive;

use crate::ToBytes;

use super::Codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u8)]
pub enum UserOption {
    Request = 0,
    Option1 = 1,
    Option2 = 2,
    Option3 = 3,
    Option4 = 4,
    Option5 = 5,
    Option6 = 6,
    Option7 = 7,
    Option8 = 8,
}

#[derive(Debug)]
pub struct OptionToggle {
    pub user_option: UserOption,
}

impl ToBytes for OptionToggle {
    const OPCODE: u8 = Codes::OptionToggle as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.user_option.into());
    }
}
