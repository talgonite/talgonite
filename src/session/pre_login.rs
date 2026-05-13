use anyhow::anyhow;
use async_std::future::timeout;
use async_std::net::TcpStream;
use async_std::sync::Arc;
use std::time::Duration;
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
    fn format_login_message(message: &server::LoginMessage) -> String {
        match message.msg_type {
            server::LoginMessageType::Other(code) => {
                format!("Unknown login message code {}: {}", code, message.msg)
            }
            msg_type => format!("{:?} (code {}): {}", msg_type, msg_type.code(), message.msg),
        }
    }

    async fn flush_login_prelude(&mut self) -> Result<(), LoginError> {
        loop {
            let packet = match timeout(Duration::from_millis(200), self.decoder.read()).await {
                Ok(Ok(packet)) => packet,
                Ok(Err(error)) => {
                    return Err(LoginError::Network(format!(
                        "Failed to read login prelude packet: {error}"
                    )))
                }
                Err(_) => break,
            };

            let description = self.describe_prelogin_packet(&packet);
            tracing::info!("Processing prelogin prelude packet: {}", description);

            match server::Codes::try_from(packet[0]) {
                Ok(server::Codes::LoginNotice) => {
                    let mut payload = packet[1..].to_vec();
                    let notice = server::LoginNotice::try_from_bytes(
                        &self.decrypter.decrypt(&mut payload, EncryptionType::Normal),
                    )
                    .map_err(|_| LoginError::Unknown)?;

                    if matches!(notice, server::LoginNotice::CheckSum { .. }) {
                        self.sender
                            .send_packet(&client::NoticeRequest)
                            .await
                            .map_err(|_| {
                                LoginError::Network(
                                    "Failed to send login notice request".to_string(),
                                )
                            })?;
                        self.sender.flush().await.map_err(|_| {
                            LoginError::Network(
                                "Failed to flush login notice request".to_string(),
                            )
                        })?;
                    }
                }
                Ok(server::Codes::LoginControl) => {
                    self.sender
                        .send_packet(&client::HomepageRequest)
                        .await
                        .map_err(|_| {
                            LoginError::Network("Failed to send homepage request".to_string())
                        })?;
                    self.sender.flush().await.map_err(|_| {
                        LoginError::Network("Failed to flush homepage request".to_string())
                    })?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn describe_prelogin_packet(&self, packet: &[u8]) -> String {
        let opcode = packet.first().copied().unwrap_or_default();

        match server::Codes::try_from(opcode) {
            Ok(server::Codes::LoginMessage) => {
                let mut payload = packet[1..].to_vec();
                match server::LoginMessage::try_from_bytes(
                    &self.decrypter.decrypt(&mut payload, EncryptionType::Normal),
                ) {
                    Ok(message) => format!(
                        "LoginMessage(type={:?}, code={}, msg={:?})",
                        message.msg_type,
                        message.msg_type.code(),
                        message.msg
                    ),
                    Err(error) => format!("LoginMessage(parse_error={error:?})"),
                }
            }
            Ok(server::Codes::LoginNotice) => {
                let mut payload = packet[1..].to_vec();
                match server::LoginNotice::try_from_bytes(
                    &self.decrypter.decrypt(&mut payload, EncryptionType::Normal),
                ) {
                    Ok(notice) => format!("LoginNotice({notice:?})"),
                    Err(error) => format!("LoginNotice(parse_error={error:?})"),
                }
            }
            Ok(server::Codes::LoginControl) => {
                let mut payload = packet[1..].to_vec();
                match server::LoginControl::try_from_bytes(
                    &self.decrypter.decrypt(&mut payload, EncryptionType::Normal),
                ) {
                    Ok(control) => format!(
                        "LoginControl(type={:?}, message={:?})",
                        control.login_controls_type, control.message
                    ),
                    Err(error) => format!("LoginControl(parse_error={error:?})"),
                }
            }
            Ok(code) => format!("{:?}(opcode={}, len={})", code, opcode, packet.len()),
            Err(_) => format!("Unknown(opcode={}, len={})", opcode, packet.len()),
        }
    }

    async fn read_login_message(&mut self, read_error: &str) -> Result<server::LoginMessage, LoginError> {
        let mut last_packet_description = None;

        loop {
            let mut packet = self
                .decoder
                .read()
                .await
                .map_err(|error| {
                    let detail = last_packet_description
                        .map(|packet| format!("{read_error}: {error} (last packet: {packet})"))
                        .unwrap_or_else(|| format!("{read_error}: {error}"));
                    LoginError::Network(detail)
                })?;

            let packet_description = self.describe_prelogin_packet(&packet);
            last_packet_description = Some(packet_description.clone());

            if packet[0] != server::Codes::LoginMessage as u8 {
                tracing::info!(
                    "Ignoring prelogin packet while waiting for login message: {}",
                    packet_description
                );
                continue;
            }

            let message = server::LoginMessage::try_from_bytes(
                &self
                    .decrypter
                    .decrypt(&mut packet[1..], EncryptionType::Normal),
            )
            .map_err(|error| {
                LoginError::Network(format!(
                    "{read_error}: failed to parse login message from {packet_description}: {error}"
                ))
            })?;
            tracing::info!(
                "Received prelogin login message while waiting for response: {}",
                Self::format_login_message(&message)
            );
            return Ok(message);
        }
    }

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
        let mut session = Self {
            decoder,
            decrypter: PacketDecrypter::new(redirect.key, redirect.seed),
            sender,
        };
        session
            .flush_login_prelude()
            .await
            .map_err(|error| anyhow!("{error:?}"))?;
        Ok(session)
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
        self.sender
            .flush()
            .await
            .map_err(|_| LoginError::Network("Failed to flush login packet".to_string()))?;

        let login_response = self.read_login_message("Failed to read login response").await?;

        if login_response.msg_type != LoginMessageType::Confirm {
            return Err(match login_response.msg_type {
                LoginMessageType::Other(_) => LoginError::Network(format!(
                    "Login rejected: {}",
                    Self::format_login_message(&login_response)
                )),
                msg_type => LoginError::Response(msg_type),
            });
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
        gender: client::CharGender,
        hair_color: u8,
    ) -> Result<(), LoginError> {
        self.sender
            .send_packet(&client::CreateCharInitial {
                name: name.to_string(),
                password: password.to_string(),
                email: "".to_string(),
            })
            .await
            .map_err(|_| {
                LoginError::Network("Failed to send character creation packet".to_string())
            })?;
        self.sender
            .flush()
            .await
            .map_err(|_| {
                LoginError::Network("Failed to flush character creation packet".to_string())
            })?;

        let initial_response = self
            .read_login_message("Failed to read character creation response")
            .await?;
        if initial_response.msg_type != LoginMessageType::Confirm {
            return Err(match initial_response.msg_type {
                LoginMessageType::Other(_) => LoginError::Network(format!(
                    "Character creation rejected: {}",
                    Self::format_login_message(&initial_response)
                )),
                msg_type => LoginError::Response(msg_type),
            });
        }

        self.sender
            .send_packet(&client::CreateCharFinalize {
                hair_style,
                gender,
                hair_color,
            })
            .await
            .map_err(|_| {
                LoginError::Network("Failed to send character finalize packet".to_string())
            })?;
        self.sender
            .flush()
            .await
            .map_err(|_| {
                LoginError::Network("Failed to flush character finalize packet".to_string())
            })?;

        let finalize_response = self
            .read_login_message("Failed to read character finalize response")
            .await?;
        if finalize_response.msg_type != LoginMessageType::Confirm {
            return Err(match finalize_response.msg_type {
                LoginMessageType::Other(_) => LoginError::Network(format!(
                    "Character creation finalize rejected: {}",
                    Self::format_login_message(&finalize_response)
                )),
                msg_type => LoginError::Response(msg_type),
            });
        }

        Ok(())
    }
}
