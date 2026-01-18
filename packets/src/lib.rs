pub mod client;
pub mod server;
pub mod types;

pub trait FromBytes {
    fn from_bytes(bytes: &[u8]) -> Self
    where
        Self: Sized;
}

pub trait TryFromBytes {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized;
}

pub trait ToBytes {
    const OPCODE: u8;

    fn write_payload(&self, bytes: &mut Vec<u8>);

    fn write_to(&self, buf: &mut Vec<u8>) {
        buf.push(Self::OPCODE);
        self.write_payload(buf);
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![];
        self.write_to(&mut buf);
        buf
    }
}

pub fn dialog_encrypt(data: &[u8]) -> Vec<u8> {
    use crc::crc16;
    use rand::Rng;

    let mut rng = rand::rng();
    let r1: u8 = rng.random();
    let r2: u8 = rng.random();

    let checksum = crc16(data);
    let len_minus_4 = (data.len() + 2) as u16;

    let mut buffer = Vec::with_capacity(data.len() + 6);
    buffer.extend_from_slice(&[r1, r2]);
    buffer.extend_from_slice(&len_minus_4.to_be_bytes());
    buffer.extend_from_slice(&checksum.to_be_bytes());
    buffer.extend_from_slice(data);

    let key = r2 ^ r1.wrapping_sub(45);
    let len_xor = key.wrapping_add(114);
    let data_xor = key.wrapping_add(40);

    buffer[2] ^= len_xor;
    buffer[3] ^= len_xor.wrapping_add(1);

    for (i, b) in buffer[4..].iter_mut().enumerate() {
        *b ^= data_xor.wrapping_add(i as u8);
    }

    buffer
}
