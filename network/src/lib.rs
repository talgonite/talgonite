pub mod cipher;
pub mod packet;
pub mod protocol;

use packets::ToBytes;
use std::io;

use self::cipher::{PacketDecrypter, PacketEncrypter};
use self::packet::{PacketDecoder, PacketEncoder};
use self::protocol::{EncryptionType, PACKET_MAGIC};

pub struct EncryptedSender {
    encoder: PacketEncoder,
    encrypter: PacketEncrypter,
}

pub struct DecryptedReceiver {
    decoder: PacketDecoder,
    decrypter: PacketDecrypter,
}

impl EncryptedSender {
    pub fn new(encoder: PacketEncoder, encrypter: PacketEncrypter) -> Self {
        Self { encoder, encrypter }
    }

    pub fn encrypt(&mut self, data: &[u8]) -> Vec<u8> {
        let enc_type = self.get_encryption_type(data[0]);
        self.encrypter.encrypt(data, enc_type)
    }

    pub async fn send(&mut self, data: &[u8]) -> io::Result<()> {
        let encoded = self.encrypt_with_header(data);
        self.send_raw(&encoded).await
    }

    pub async fn send_packet<T: ToBytes>(&mut self, packet: &T) -> io::Result<()> {
        self.send(&packet.to_bytes()).await
    }

    pub fn encrypt_with_header(&mut self, data: &[u8]) -> Vec<u8> {
        let data = self.encrypt(data);
        let len = data.len();
        let mut packet = Vec::with_capacity(3 + len);
        packet.push(PACKET_MAGIC);
        packet.push((len >> 8) as u8);
        packet.push(len as u8);
        packet.extend_from_slice(&data);
        packet
    }

    pub async fn send_raw(&mut self, data: &[u8]) -> io::Result<()> {
        self.encoder.write_raw(data).await
    }

    pub async fn flush(&mut self) -> io::Result<()> {
        self.encoder.flush().await
    }

    fn get_encryption_type(&self, opcode: u8) -> EncryptionType {
        match opcode {
            0 => EncryptionType::None,
            16 => EncryptionType::None,
            72 => EncryptionType::None,
            2 => EncryptionType::Normal,
            3 => EncryptionType::Normal,
            4 => EncryptionType::Normal,
            11 => EncryptionType::Normal,
            38 => EncryptionType::Normal,
            45 => EncryptionType::Normal,
            58 => EncryptionType::Normal,
            66 => EncryptionType::Normal,
            67 => EncryptionType::Normal,
            75 => EncryptionType::Normal,
            87 => EncryptionType::Normal,
            98 => EncryptionType::Normal,
            104 => EncryptionType::Normal,
            113 => EncryptionType::Normal,
            115 => EncryptionType::Normal,
            123 => EncryptionType::Normal,
            _ => EncryptionType::Md5,
        }
    }
}

impl DecryptedReceiver {
    pub fn new(decoder: PacketDecoder, decrypter: PacketDecrypter) -> Self {
        Self { decoder, decrypter }
    }

    pub async fn receive(&mut self) -> io::Result<(u8, Vec<u8>)> {
        let mut data = self.decoder.read().await?;
        let opcode = data[0];
        let payload = &mut data[1..];
        let enc_type = self.get_encryption_type(opcode);

        Ok(match enc_type {
            EncryptionType::None => (opcode, payload.to_vec()),
            EncryptionType::Normal => (
                opcode,
                self.decrypter
                    .decrypt(payload, EncryptionType::Normal)
                    .to_vec(),
            ),
            EncryptionType::Md5 => (
                opcode,
                self.decrypter
                    .decrypt(payload, EncryptionType::Md5)
                    .to_vec(),
            ),
        })
    }

    fn get_encryption_type(&self, opcode: u8) -> EncryptionType {
        match opcode {
            0 => EncryptionType::None,
            3 => EncryptionType::None,
            64 => EncryptionType::None,
            126 => EncryptionType::None,
            1 => EncryptionType::Normal,
            2 => EncryptionType::Normal,
            10 => EncryptionType::Normal,
            86 => EncryptionType::Normal,
            96 => EncryptionType::Normal,
            98 => EncryptionType::Normal,
            102 => EncryptionType::Normal,
            111 => EncryptionType::Normal,
            _ => EncryptionType::Md5,
        }
    }
}
