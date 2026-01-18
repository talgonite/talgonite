use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct SocialStatus {
    pub social_status: u8,
}

impl ToBytes for SocialStatus {
    const OPCODE: u8 = Codes::SocialStatus as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.social_status);
    }
}
