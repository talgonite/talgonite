use anyhow::anyhow;
use async_std::net::TcpStream;
use async_std::sync::Arc;
use packets::{
    ToBytes, TryFromBytes, client,
    server::{self, LoginMessageType},
};

use network::{
    DecryptedReceiver, EncryptedSender,
    cipher::{PacketDecrypter, PacketEncrypter},
    packet::{PacketDecoder, PacketEncoder},
    protocol::EncryptionType,
};

const VERSION: u16 = 741;

pub struct PreLoginSession {
    decoder: PacketDecoder,
    decrypter: PacketDecrypter,
    sender: EncryptedSender,
}

pub use game_ui::LoginError;

impl PreLoginSession {
    pub async fn new(server_address: &str, server_port: u16) -> anyhow::Result<Self> {
        tracing::info!(
            "Connecting to lobby server at {}:{}...",
            server_address,
            server_port
        );
        let connection_string = format!("{}:{}", server_address, server_port);
        let stream = TcpStream::connect(&connection_string).await?;
        stream.set_nodelay(true).ok();
        tracing::info!("Connected to lobby server.");
        let stream = Arc::new(stream);
        let mut decoder = PacketDecoder::new(stream.clone());
        let mut encoder = PacketEncoder::new(stream);

        tracing::info!("Waiting for initial packet...");
        let packet = decoder.read().await?;
        assert_eq!(packet[0], 0x7E);
        tracing::info!("Initial packet received.");

        encoder
            .write(&client::Version { version: VERSION }.to_bytes())
            .await?;
        encoder.flush().await?;

        tracing::info!("Waiting for connection info...");
        let packet = decoder.read().await?;
        assert_eq!(packet[0], server::Codes::ConnectionInfo as u8);
        tracing::info!("Connection info received.");

        let connection_info = server::ConnectionInfo::try_from_bytes(&packet[1..])?;
        let (encryption_key, seed) = match connection_info {
            server::ConnectionInfo::Ok {
                encryption_key,
                seed,
                ..
            } => (encryption_key, seed),
            _ => return Err(anyhow!("Invalid crypto key response")),
        };

        let mut sender = EncryptedSender::new(encoder, PacketEncrypter::new(encryption_key, seed));

        sender
            .send_packet(&client::ServerTableRequest::ServerId(0))
            .await?;
        sender.flush().await?;

        let packet = decoder.read().await?;
        assert_eq!(packet[0], server::Codes::Redirect as u8);

        let redirect = match server::Redirect::try_from_bytes(&packet[1..]) {
            Ok(r) => r,
            Err(err) => return Err(anyhow!("Failed to parse Redirect packet: {:?}", err)),
        };
        let redirect_response = client::ClientRedirected {
            seed: redirect.seed,
            key: redirect.key.clone(),
            name: redirect.name,
            id: redirect.id,
        };

        let stream = TcpStream::connect(redirect.addr).await?;
        stream.set_nodelay(true).ok();
        let stream = Arc::new(stream);
        let mut decoder = PacketDecoder::new(stream.clone());
        let mut encoder = PacketEncoder::new(stream);

        let packet = decoder.read().await?;
        assert_eq!(packet[0], 0x7E);

        encoder.write(&redirect_response.to_bytes()).await?;
        encoder.flush().await?;

        let sender = EncryptedSender::new(
            encoder,
            PacketEncrypter::new(redirect.key.clone(), redirect.seed),
        );
        Ok(Self {
            decoder,
            decrypter: PacketDecrypter::new(redirect.key, redirect.seed),
            sender,
        })
    }

    pub async fn login(
        mut self,
        username: &str,
        password: &str,
    ) -> Result<(DecryptedReceiver, EncryptedSender), LoginError> {
        self.sender
            .send_packet(&client::Login {
                user: username.into(),
                pass: password.into(),
            })
            .await
            .map_err(|_| LoginError::Network("Failed to send login packet".to_string()))?;
        self.sender.flush().await.ok();

        let login_response = loop {
            let mut packet =
                self.decoder.read().await.map_err(|_| {
                    LoginError::Network("Failed to read login response".to_string())
                })?;

            if packet[0] == server::Codes::LoginMessage as u8 {
                break match server::LoginMessage::try_from_bytes(
                    &self
                        .decrypter
                        .decrypt(&mut packet[1..], EncryptionType::Normal),
                ) {
                    Ok(m) => m,
                    Err(_) => return Err(LoginError::Unknown),
                };
            }
        };

        if login_response.msg_type != LoginMessageType::Confirm {
            return Err(LoginError::Response(login_response.msg_type));
        }

        let packet = self
            .decoder
            .read()
            .await
            .map_err(|_| LoginError::Network("Failed to read redirect packet".to_string()))?;
        assert_eq!(packet[0], server::Codes::Redirect as u8);

        let redirect = match server::Redirect::try_from_bytes(&packet[1..]) {
            Ok(r) => r,
            Err(_) => return Err(LoginError::Unknown),
        };

        let redirect_response = client::ClientRedirected {
            seed: redirect.seed,
            key: redirect.key.clone(),
            name: redirect.name.clone(),
            id: redirect.id,
        };

        let stream = TcpStream::connect(redirect.addr).await.unwrap();
        stream.set_nodelay(true).ok();
        let stream = Arc::new(stream);
        let mut encoder = PacketEncoder::new(stream.clone());

        encoder.write(&redirect_response.to_bytes()).await.unwrap();
        encoder.flush().await.unwrap();

        Ok((
            DecryptedReceiver::new(
                PacketDecoder::new(stream),
                PacketDecrypter::new_with_special_key_table(
                    redirect.key.clone(),
                    redirect.seed,
                    &redirect.name,
                ),
            ),
            EncryptedSender::new(
                encoder,
                PacketEncrypter::new_with_special_key_table(
                    redirect.key,
                    redirect.seed,
                    &redirect.name,
                ),
            ),
        ))
    }

    pub async fn create_character(
        &mut self,
        name: &str,
        password: &str,
        hair_style: u8,
        gender: u8,
        hair_color: u8,
    ) -> Result<(), u8> {
        self.sender
            .send_packet(&client::CreateCharInitial {
                name: name.to_string(),
                password: password.to_string(),
            })
            .await
            .unwrap();

        // TODO: The server might send a response to CreateCharInitial.
        // We need to handle that here. For now, we'll just assume it's successful.

        self.sender
            .send_packet(&client::CreateCharFinalize {
                hair_style,
                gender,
                hair_color,
            })
            .await
            .unwrap();

        // TODO: The server will likely send a response to CreateCharFinalize.
        // We need to handle that here and return Ok(()) on success, or Err(error_code) on failure.
        // For now, we'll just assume it's successful.

        Ok(())
    }
}
