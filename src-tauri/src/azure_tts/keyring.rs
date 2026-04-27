use crate::tts::TtsError;

const SERVICE: &str = "Capture2TextPro";
const USERNAME: &str = "azure_tts_subscription_key";

pub fn save_key(key: &str) -> Result<(), TtsError> {
    entry()?
        .set_password(key)
        .map_err(|err| TtsError::Keyring(err.to_string()))
}

pub fn get_key() -> Result<Option<String>, TtsError> {
    match entry()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(::keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(TtsError::Keyring(err.to_string())),
    }
}

pub fn delete_key() -> Result<(), TtsError> {
    match entry()?.delete_credential() {
        Ok(()) | Err(::keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(TtsError::Keyring(err.to_string())),
    }
}

pub fn has_key() -> Result<bool, TtsError> {
    get_key().map(|key| key.is_some())
}

fn entry() -> Result<::keyring::Entry, TtsError> {
    ::keyring::Entry::new(SERVICE, USERNAME).map_err(|err| TtsError::Keyring(err.to_string()))
}
