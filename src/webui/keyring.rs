use anyhow::{Result, anyhow};

const SERVICE: &str = "talgonite";

pub fn set_password(cred_id: &str, password: &str) -> Result<()> {
    keyring::Entry::new(SERVICE, cred_id)
        .map_err(|e| anyhow!("keyring error: {}", e))?
        .set_password(password)
        .map_err(|e| anyhow!("failed to store password: {}", e))
}

pub fn get_password(cred_id: &str) -> Result<String> {
    keyring::Entry::new(SERVICE, cred_id)
        .map_err(|e| anyhow!("keyring error: {}", e))?
        .get_password()
        .map_err(|e| anyhow!("password not found: {}", e))
}

pub fn delete_password(cred_id: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, cred_id)
        .map_err(|e| anyhow!("keyring error: {}", e))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(anyhow!("failed to delete password: {}", e)),
    }
}
