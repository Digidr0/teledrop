use secrecy::SecretString;

fn entry() -> anyhow::Result<keyring::Entry> {
    Ok(keyring::Entry::new("TeleDrop", "telegram-bot-token")?)
}

pub fn load() -> anyhow::Result<Option<SecretString>> {
    match entry()?.get_password() {
        Ok(token) => Ok(Some(SecretString::from(token))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => anyhow::bail!("Windows Credential Manager недоступен."),
    }
}

pub fn save(token: &str) -> anyhow::Result<()> {
    entry()?
        .set_password(token)
        .map_err(|_| anyhow::anyhow!("Не удалось сохранить токен в Credential Manager."))
}

pub fn remove() -> anyhow::Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => anyhow::bail!("Не удалось удалить токен."),
    }
}

/// Do not log transport errors: Reqwest includes the credential-bearing URL.
pub fn transport_message(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "Telegram не ответил вовремя."
    } else if error.is_connect() {
        "Нет соединения с Telegram."
    } else {
        "Ошибка передачи данных Telegram."
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Explicit native Windows Credential Manager integration; uses its own isolated test entry"]
    fn windows_credential_roundtrip() {
        struct Cleanup(keyring::Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let service = format!("TeleDrop.Test.{}", uuid::Uuid::new_v4());
        let credential = Cleanup(keyring::Entry::new(&service, "isolated-test").unwrap());
        assert!(matches!(
            credential.0.get_password(),
            Err(keyring::Error::NoEntry)
        ));
        credential
            .0
            .set_password("TEST_ONLY_NOT_A_REAL_TOKEN")
            .unwrap();
        assert_eq!(
            credential.0.get_password().unwrap(),
            "TEST_ONLY_NOT_A_REAL_TOKEN"
        );
        credential.0.delete_credential().unwrap();
        assert!(matches!(
            credential.0.get_password(),
            Err(keyring::Error::NoEntry)
        ));
    }
}
