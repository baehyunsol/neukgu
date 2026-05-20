// VIBE NOTE: gemma-4-26B-A4B (via neukgu) wrote this code.
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use rand::RngCore;

#[derive(Debug)]
pub enum Error {
    DecryptionError,
    KeyDerivationError,
}

/// Derives a secure 32-byte key from a password and a salt using Argon2.
/// In a production system, you would store the salt alongside the ciphertext.
fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; 32], Error> {
    let mut key = [0u8; 32];
    let argon2 = Argon2::default();

    // We use Argon2 to turn a human password into a high-entropy cryptographic key
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|_| Error::KeyDerivationError)?;

    Ok(key)
}

/// Encrypts data.
/// The returned Vec contains: [SALT (16 bytes)] + [NONCE (12 bytes)] + [CIPHERTEXT]
pub fn encrypt(data: &[u8], password: &[u8]) -> Vec<u8> {
    let mut rng = rand::rng();

    // 1. Generate a random salt for key derivation
    let mut salt = [0u8; 16];
    rng.fill_bytes(&mut salt);

    // 2. Derive the key from the password
    let key_bytes = derive_key(password, &salt).expect("Key derivation failed");
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).expect("Invalid key length");

    // 3. Generate a random Nonce (Initialization Vector)
    let mut nonce_bytes = [0u8; 12];
    rng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // 4. Encrypt the data
    let ciphertext = cipher
        .encrypt(nonce, data)
        .expect("Encryption failed");

    // 5. Package everything together so we can decrypt it later
    let mut output = Vec::new();
    output.extend_from_slice(&salt);       // Store salt to re-derive key
    output.extend_from_slice(&nonce_bytes); // Store nonce to decrypt
    output.extend_from_slice(&ciphertext); // The actual secret
    output
}

/// Decrypts data.
pub fn decrypt(data: &[u8], password: &[u8]) -> Result<Vec<u8>, Error> {
    // Minimum length check: Salt(16) + Nonce(12) + Tag(16)
    if data.len() < 44 {
        return Err(Error::DecryptionError);
    }

    // 1. Split the input into its components
    let salt = &data[0..16];
    let nonce_bytes = &data[16..28];
    let ciphertext = &data[28..];

    // 2. Re-derive the same key using the extracted salt
    let key_bytes = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|_| Error::DecryptionError)?;
    let nonce = Nonce::from_slice(nonce_bytes);

    // 3. Decrypt and verify authenticity
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| Error::DecryptionError)?;

    Ok(plaintext)
}
