//! The least version of Muddie a plugin asks for, read on its own before
//! the manifest is, so that a plugin with fields this Muddie does not know
//! can still say it is for a newer one.

/// The numbers of a version, as `0.4.0`: a part that is not a number, and
/// what follows it (`-beta`, `+build`), is not counted. `None` if there is
/// not even a first number.
fn numbers(version: &str) -> Option<[u64; 3]> {
    let mut found = [0u64; 3];
    for (index, part) in version.trim().split('.').take(3).enumerate() {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        match digits.parse() {
            Ok(number) => found[index] = number,
            Err(_) if index == 0 => return None,
            Err(_) => break,
        }
        if digits.len() != part.len() {
            break;
        }
    }
    Some(found)
}

/// Whether `required` is a later version than `current`. A version that
/// cannot be read asks for nothing.
pub fn is_newer(required: &str, current: &str) -> bool {
    match (numbers(required), numbers(current)) {
        (Some(required), Some(current)) => required > current,
        _ => false,
    }
}

/// The version of Muddie that `manifest`, the text of a `plugin.toml`, asks
/// for, if it says one: read without the manifest being understood whole.
pub fn required(manifest: &str) -> Option<String> {
    let table: toml::Table = toml::from_str(manifest).ok()?;
    Some(table.get("muddie")?.as_str()?.trim().to_owned())
}

/// This Muddie's version.
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_compared_by_their_numbers() {
        assert!(is_newer("0.4.0", "0.3.0"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(!is_newer("0.3.0", "0.3.0"));
        assert!(!is_newer("0.3.0", "0.4.0"));
        assert!(!is_newer("0.4.0-beta.1", "0.4.0"));
        assert!(!is_newer("soon", "0.4.0"));
        assert!(!is_newer("", "0.4.0"));
    }

    #[test]
    fn the_version_is_read_though_the_rest_is_not_understood() {
        let future = "muddie = \"0.4.0\"\nname = \"X\"\nbrand_new_field = [1, 2]\n";
        assert_eq!(required(future).as_deref(), Some("0.4.0"));
        assert_eq!(required("name = \"X\"\n"), None);
        assert_eq!(required("muddie = 4\n"), None);
        assert_eq!(required("= not toml"), None);
    }
}
