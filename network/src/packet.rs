use super::protocol::{HEADER_SIZE, PACKET_MAGIC};
use async_std::io::{ReadExt, WriteExt};
use async_std::net::TcpStream;
use async_std::sync::Arc;
use std::io;

pub struct PacketDecoder {
    stream: Arc<TcpStream>,
}

impl PacketDecoder {
    pub fn new(stream: Arc<TcpStream>) -> Self {
        Self { stream }
    }

    pub async fn read(&mut self) -> io::Result<Vec<u8>> {
        let mut header = [0; HEADER_SIZE];
        if let Err(e) = (&*self.stream).read_exact(&mut header).await {
            return Err(e);
        }

        let magic_val = header[0];
        let length = u16::from_be_bytes([header[1], header[2]]) as usize;

        if magic_val != PACKET_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid packet magic: {:#04x}", magic_val),
            ));
        }

        let mut buf = vec![0; length];
        if let Err(e) = (&*self.stream).read_exact(&mut buf).await {
            return Err(e);
        }

        Ok(buf)
    }
}

pub struct PacketEncoder {
    stream: Arc<TcpStream>,
}

impl PacketEncoder {
    pub fn new(stream: Arc<TcpStream>) -> Self {
        Self { stream }
    }

    pub async fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let total_len = data.len();
        if total_len > u16::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "packet data too large",
            ));
        }

        let mut packet = Vec::with_capacity(HEADER_SIZE + total_len);
        packet.push(PACKET_MAGIC);
        packet.push((total_len >> 8) as u8);
        packet.push(total_len as u8);
        packet.extend_from_slice(data);

        self.write_raw(&packet).await
    }

    pub async fn write_raw(&mut self, data: &[u8]) -> io::Result<()> {
        (&*self.stream).write_all(data).await
    }

    pub async fn flush(&mut self) -> io::Result<()> {
        (&*self.stream).flush().await
    }
}
