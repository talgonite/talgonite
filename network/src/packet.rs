use super::protocol::{HEADER_SIZE, PACKET_MAGIC};
use async_std::io::{ReadExt, WriteExt};
use async_std::net::TcpStream;
use std::io;
use async_std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct PacketDecoder {
    stream: Arc<Mutex<TcpStream>>,
}

impl PacketDecoder {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
    }

    pub async fn read(&mut self) -> io::Result<Vec<u8>> {
        let mut stream = self.stream.lock().await;

        let mut header = [0; HEADER_SIZE];
        stream.read_exact(&mut header).await?;

        let magic_val = header[0];
        let length = u16::from_be_bytes([header[1], header[2]]) as usize;

        if magic_val != PACKET_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid packet magic",
            ));
        }

        let mut buf = vec![0; length];
        stream.read_exact(&mut buf).await?;

        Ok(buf)
    }
}

#[derive(Clone)]
pub struct PacketEncoder {
    stream: Arc<Mutex<TcpStream>>,
}

impl PacketEncoder {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
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

        self.stream.lock().await.write_all(&packet).await
    }
}
