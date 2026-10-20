use keyring::Entry;

const SERVICE: &str = "ni";

pub(crate) fn get(provider: &str) -> Option<String> {
    let entry = Entry::new(SERVICE, provider).ok()?;
    entry
        .get_password()
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn set(provider: &str, api_key: &str) -> bool {
    Entry::new(SERVICE, provider)
        .and_then(|entry| entry.set_password(api_key))
        .is_ok()
}

pub(crate) fn delete(provider: &str) -> bool {
    Entry::new(SERVICE, provider)
        .and_then(|entry| entry.delete_credential())
        .is_ok()
}
