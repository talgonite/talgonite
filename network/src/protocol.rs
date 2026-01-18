pub const PACKET_MAGIC: u8 = 0xaa;
pub const HEADER_SIZE: usize = 3;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum EncryptionType {
    None,
    Normal,
    Md5,
}
