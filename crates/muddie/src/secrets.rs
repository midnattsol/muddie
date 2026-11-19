//! Passwords, kept in the operating system's keyring rather than in a file.

use keyring::Entry;

/// Name the passwords are filed under in the keyring.
const SERVICE: &str = "muddie";

/// The password saved for a profile, if there is one.
pub fn password(profile: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(SERVICE, profile).map_err(|error| error.to_string())?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Saves a profile's password, or forgets it if `password` is `None`.
pub fn set_password(profile: &str, password: Option<&str>) -> Result<(), String> {
    let entry = Entry::new(SERVICE, profile).map_err(|error| error.to_string())?;
    let result = match password {
        Some(password) => entry.set_password(password),
        None => match entry.delete_credential() {
            Err(keyring::Error::NoEntry) => Ok(()),
            other => other,
        },
    };
    result.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uses the real keyring of whoever runs it, so it is not part of the
    /// normal test run: `cargo test -p muddie -- --ignored keyring`.
    #[test]
    #[ignore = "needs a desktop session with a keyring"]
    fn keyring_stores_and_forgets_a_password() {
        let profile = "muddie self-test entry";
        set_password(profile, Some("s3cret")).unwrap();
        assert_eq!(password(profile).unwrap().as_deref(), Some("s3cret"));
        set_password(profile, None).unwrap();
        assert_eq!(password(profile).unwrap(), None);
        set_password(profile, None).unwrap();
    }
}
