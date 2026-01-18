use super::protocol::EncryptionType;

#[derive(Clone)]
pub struct PacketDecrypter {
    key: Vec<u8>,
    special_key_table: Option<Vec<u8>>,
    cipher: PacketCipher,
}

impl PacketDecrypter {
    pub fn new(key: Vec<u8>, seed: u8) -> Self {
        Self {
            key,
            cipher: PacketCipher::new(seed),
            special_key_table: None,
        }
    }

    pub fn new_with_special_key_table(key: Vec<u8>, seed: u8, key_salt_seed: &str) -> Self {
        Self {
            key,
            cipher: PacketCipher::new(seed),
            special_key_table: Some(generate_special_key_table(key_salt_seed)),
        }
    }

    pub fn new_with_cipher(key: Vec<u8>, cipher: PacketCipher) -> Self {
        Self {
            key,
            cipher,
            special_key_table: None,
        }
    }

    fn get_key(&self, a: u16, b: u8, enc_type: EncryptionType) -> Vec<u8> {
        if enc_type == EncryptionType::Normal {
            return self.key.clone();
        }

        match &self.special_key_table {
            Some(special_key_table) => (0..9)
                .map(|i| {
                    let index = ((i as usize * (9 * i as usize + b as usize * b as usize)
                        + a as usize)
                        % special_key_table.len()) as usize;
                    special_key_table[index]
                })
                .collect(),
            None => self.key.clone(),
        }
    }

    pub fn decrypt<'a>(&self, data: &'a mut [u8], enc_type: EncryptionType) -> &'a mut [u8] {
        let ordinal = data[0];
        let len = data.len();
        let a = (((data[len - 1] as u16) << 8) | data[len - 3] as u16) ^ 0x6474;
        let b = data[len - 2] ^ 0x24;

        let payload = &mut data[1..len - 3];

        self.cipher
            .crypt(payload, ordinal, &self.get_key(a, b, enc_type));

        payload
    }

    pub fn decrypt_with_a_b<'a>(
        &self,
        data: &'a mut [u8],
        enc_type: EncryptionType,
    ) -> (Vec<u8>, u16, u8) {
        let ordinal = data[0];
        let len = data.len();
        let a = (((data[len - 1] as u16) << 8) | data[len - 3] as u16) ^ 0x6474;
        let b = data[len - 2] ^ 0x24;

        let payload = &mut data[1..len - 3];

        self.cipher
            .crypt(payload, ordinal, &self.get_key(a, b, enc_type));

        (payload.to_vec(), a, b)
    }
}

#[derive(Clone)]
pub struct PacketEncrypter {
    key: Vec<u8>,
    cipher: PacketCipher,
    special_key_table: Option<Vec<u8>>,
    rand_state: u32,
    ordinal: u8,
}

impl PacketEncrypter {
    pub fn new(key: Vec<u8>, seed: u8) -> Self {
        Self {
            key,
            cipher: PacketCipher::new(seed),
            special_key_table: None,
            rand_state: 1,
            ordinal: 0,
        }
    }

    pub fn new_with_special_key_table(key: Vec<u8>, salt_seed: u8, special_key_seed: &str) -> Self {
        Self {
            key,
            cipher: PacketCipher::new(salt_seed),
            special_key_table: Some(generate_special_key_table(special_key_seed)),
            rand_state: 1,
            ordinal: 0,
        }
    }

    pub(crate) fn encrypt_with_random(
        &self,
        ordinal: u8,
        data: &[u8],
        special_key_seed: u16,
        enc_type: EncryptionType,
    ) -> Vec<u8> {
        let opcode = data[0];

        let mut a = ((special_key_seed % 65277) + 256) as u16;
        let mut b = 100u8;

        let mut data = data.to_vec();

        data.push(0);
        if self.special_key_table.is_some() {
            data.push(opcode);
        }

        self.cipher
            .crypt(&mut data[1..], ordinal, &self.get_key(a, b, enc_type));

        data.insert(1, ordinal);

        let hash = md5::compute(&data);

        a ^= 0x7470;
        b ^= 0x23;

        data.extend_from_slice(&[
            hash[13],
            hash[3],
            hash[11],
            hash[7],
            a as u8,
            b,
            (a >> 8) as u8,
        ]);

        data
    }

    pub fn encrypt(&mut self, data: &[u8], enc_type: EncryptionType) -> Vec<u8> {
        let rand = self.next_rand();
        self.ordinal = self.ordinal.wrapping_add(1);
        self.encrypt_with_random(self.ordinal, data, rand, enc_type)
    }

    fn get_key(&self, a: u16, b: u8, enc_type: EncryptionType) -> Vec<u8> {
        if enc_type == EncryptionType::Normal {
            return self.key.clone();
        }

        match &self.special_key_table {
            Some(special_key_table) => (0..9)
                .map(|i| {
                    let index = ((i as usize * (9 * i as usize + b as usize * b as usize)
                        + a as usize)
                        % special_key_table.len()) as usize;
                    special_key_table[index]
                })
                .collect(),
            None => self.key.clone(),
        }
    }

    fn next_rand(&mut self) -> u16 {
        self.rand_state = self.rand_state.wrapping_mul(0x343FD).wrapping_add(0x269EC3);

        return ((self.rand_state >> 0x10) & 0x7FFF) as _;
    }
}

fn to_hex_string(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

fn md5_hex_string(data: &str) -> String {
    let digest = md5::compute(data.as_bytes()).0;
    to_hex_string(&digest)
}

fn generate_special_key_table(seed: &str) -> Vec<u8> {
    let mut table = md5_hex_string(&md5_hex_string(seed));

    for _ in 0..31 {
        table.push_str(&md5_hex_string(&table));
    }

    table.as_bytes().to_vec()
}

fn generate_salt(seed: u8) -> [u8; 256] {
    let mut salt = [0; 256];

    for i in 0..256i32 {
        let salt_byte = match seed {
            0 => i,
            1 => (if i % 2 != 0 { -1 } else { 1 } * ((i + 1) / 2)) + 128,
            2 => 255 - i,
            3 => (if i % 2 != 0 { -1 } else { 1 } * ((255 - i) / 2)) + 128,
            4 => (i / 16) * (i / 16),
            5 => 2 * i % 256,
            6 => 255 - 2 * i % 256,
            7 => {
                if i > 127 {
                    2 * i - 256
                } else {
                    255 - 2 * i
                }
            }
            8 => {
                if i > 127 {
                    511 - 2 * i
                } else {
                    2 * i
                }
            }
            9 => 255 - ((i - 128) / 8 * ((i - 128) / 8) % 256),
            _ => 0,
        };

        let salt_byte =
            (salt_byte | (salt_byte << 8) | ((salt_byte | (salt_byte << 8)) << 16)) as u32;
        salt[i as usize] = salt_byte as u8;
    }

    salt
}

#[derive(Clone)]
pub struct PacketCipher {
    salt_table: [u8; 256],
}

impl PacketCipher {
    pub fn new(seed: u8) -> Self {
        Self {
            salt_table: generate_salt(seed),
        }
    }

    pub fn crypt(&self, data: &mut [u8], ordinal: u8, key: &[u8]) {
        let ordinal = ordinal as usize;
        let salt = &self.salt_table;
        let key_length = key.len();
        data.iter_mut().enumerate().for_each(|(i, v)| {
            let table_key_index = (i / key_length) % salt.len();

            *v ^= salt[table_key_index] ^ key[i % key_length];

            if table_key_index != ordinal {
                *v ^= salt[ordinal];
            }
        });
    }
}
