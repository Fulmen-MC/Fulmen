//! Offline-mode player UUIDs.

use md5::{Digest, Md5};

/// The UUID a vanilla server derives from a player name in offline mode
/// (a version 3 UUID over `"OfflinePlayer:" + name`).
pub fn offline_uuid(name: &str) -> u128 {
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    hash[6] = (hash[6] & 0x0f) | 0x30;
    hash[8] = (hash[8] & 0x3f) | 0x80;
    u128::from_be_bytes(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_offline_uuids() {
        // Values computed independently with Python's hashlib.
        assert_eq!(
            offline_uuid("Notch"),
            0xb50ad385_829d_3141_a216_7e7d7539ba7f
        );
        assert_eq!(
            offline_uuid("Fulmen"),
            0x37c52288_17d0_3f7a_98de_2ae6245287b6
        );
    }
}
