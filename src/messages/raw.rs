#[cfg(feature = "raw-crypto")]
impl Message {
    /// Encrypts current message by consuming it.
    /// Uses provided cryptography function to perform
    ///     the encryption. Agnostic of actual algorythm used.
    /// Consuming is to make sure no changes are
    ///     possible post packaging / sending.
    /// Returns `Vec<u8>` to be sent to receiver.
    ///
    fn send_raw(self, crypter: fn(&[u8], &[u8]) -> Vec<u8>, receiver_pk: &[u8])
        -> Result<Vec<u8>, Error> {
            Ok(crypter(receiver_pk, serde_json::to_string(&self)?.as_bytes()))
    }
    /// Decrypts received cypher into instance of `Message`.
    /// Received message should be encrypted with our pub key.
    /// Returns `Ok(Message)` if decryption / deserialization
    ///     succeded. `Error` othervice.
    ///
    fn receive_raw(
        received_message: &[u8],
        decrypter: fn(&[u8], &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>>,
        our_sk: &[u8]) 
            -> Result<Self, Error> {
        if let Ok(raw_message_bytes) = decrypter(our_sk, received_message)
            .map_err(|e| Error::Other(e)) {
                return Ok(serde_json::from_slice(&raw_message_bytes)?);
            }
        Err(Error::PlugCryptoFailure)
    }
    /// Encrypts current message by consuming it.
    /// Uses provided cryptography function to perform
    ///     the asymmentric encryption. Agnostic of actual algorythm used.
    /// Consuming is to make sure no changes are
    ///     possible post packaging / sending.
    /// Returns `Vec<u8>` to be sent to receiver.
    ///
    fn send_asymm(
        self,
        crypter: fn(plaintext: &[u8], nonce: &[u8], their_pk: &[u8], our_sk: &[u8])
            -> Result<Vec<u8>, Box<dyn std::error::Error>>,
        nonce: &[u8],
        their_pk: &[u8],
        our_sk: &[u8]
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        crypter(&serde_json::to_string(&self)?.as_bytes(), nonce, their_pk, our_sk)
    }
    /// Decrypts received cypher into instance of `Message`.
    /// Asymmetric crypto algorythm is expected.
    /// Returns `Ok(Message)` if decryption / deserialization
    ///     succeded. `Error` othervice.
    ///
    fn receive_asymm(
        received_message: &[u8],
        decrypter: fn(received_message: &[u8], nonce: &[u8], our_pk: &[u8], their_sk: &[u8])
            -> Result<Vec<u8>, Box<dyn std::error::Error>>,
        nonce: &[u8],
        our_pk: &[u8],
        their_sk: &[u8]
    ) -> Result<Self, Error> {
        if let Ok(raw_message_bytes) = decrypter(received_message, nonce, our_pk, their_sk)
            .map_err(|e| Error::Other(e)) {
                return Ok(serde_json::from_slice(&raw_message_bytes)?);
            }
        Err(Error::PlugCryptoFailure)
    }
}

#[cfg(test, feature = "raw-crypto")] 
mod raw_tests {
    use chacha20poly1305::{XChaCha20Poly1305, Key, XNonce};
    use chacha20poly1305::aead::{Aead, NewAead};
    use sodiumoxide::crypto::{
        secretbox,
        box_
    };
    use rand_core::OsRng;
    use x25519_dalek::{
        EphemeralSecret,
        PublicKey,
    };
    
    #[test]
    #[allow(non_snake_case)]
    #[cfg(feature="raw-crypto")]
    fn plugin_crypto_xChaCha20Paly1305_dummy_key() {
        // Arrange
        let key = Key::from_slice(b"an example very very secret key.");
        // Plugable encryptor function to encrypt data
        let my_crypter = |k: &[u8], m: &[u8]| -> Vec<u8> {
            let aead = XChaCha20Poly1305::new(k.into());
            let nonce = XNonce::from_slice(b"extra long unique nonce!");
            aead.encrypt(nonce, m).expect("encryption failure!")
        };
        // Plugable decryptor function to decrypt data
        let my_decrypter = |k: &[u8], m: &[u8]| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let aead = XChaCha20Poly1305::new(k.into());
            let nonce = XNonce::from_slice(b"extra long unique nonce!");
            Ok(aead.decrypt(nonce, m).unwrap())
        };
        let m = Message::new();
        let id = m.headers.id;

        // Act and Assert
        let crypted = m.send_raw(my_crypter, key);
        assert!(&crypted.is_ok()); // Encryption check
        let raw_m = Message::receive_raw(&crypted.unwrap(), my_decrypter, key);
        assert!(&raw_m.is_ok()); // Decryption check
        assert_eq!(id, raw_m.unwrap().headers.id); // Data consistancy check
    }

    #[test]
    #[cfg(feature="raw-crypto")]
    fn plugin_crypto_libsodium_box() {
        // Arrange
        // Plugable encryptor function to encrypt data
        let my_crypter = |k: &[u8], m: &[u8]|
            -> Vec<u8> {
            let nonce = secretbox::Nonce::from_slice(b"extra long unique nonce!").unwrap();
            secretbox::seal(m, &nonce, &secretbox::Key::from_slice( k).unwrap())
        };
        // Plugable decryptor function to decrypt data
        let my_decrypter = |k: &[u8], m: &[u8]|
            -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let nonce = secretbox::Nonce::from_slice(b"extra long unique nonce!").unwrap();
            secretbox::open(m, &nonce, &secretbox::Key::from_slice(k).unwrap())
                .map_err(|_| Error::PlugCryptoFailure.into())
        };
        let m = Message::new();
        let id = m.headers.id;
        let key = secretbox::gen_key();

        // Act and Assert
        let crypted = m.send_raw(my_crypter, &key.as_ref());
        assert!(&crypted.is_ok()); // Encryption check
        let raw_m = Message::receive_raw(&crypted.unwrap(), my_decrypter, &key.as_ref());
        assert!(&raw_m.is_ok()); // Decryption check
        assert_eq!(id, raw_m.unwrap().headers.id); // Data consistancy check
    }

    #[test]
    #[cfg(feature="raw-crypto")]
    fn plugin_crypto_asymm_libsodium_box() {
        // Arrange
        let (sender_pk, sender_sk) = box_::gen_keypair();
        let (receiver_pk, receiver_sk) = box_::gen_keypair();
        // Plugable encryptor function to encrypt data
        let nonce = box_::Nonce::from_slice(b"extra long unique nonce!").unwrap();
        let my_crypter = |m: &[u8], n: &[u8], pk: &[u8], sk: &[u8]|
            -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            Ok(box_::seal(m,
                &box_::Nonce::from_slice(n).unwrap(),
                &box_::PublicKey::from_slice(pk).unwrap(),
                &box_::SecretKey::from_slice(sk).unwrap())
            )
        };
        // Plugable decryptor function to decrypt data
        let my_decrypter = |m: &[u8], n: &[u8], pk: &[u8], sk: &[u8]|
            -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            Ok(box_::open(m,
                &box_::Nonce::from_slice(n).unwrap(),
                &box_::PublicKey::from_slice(pk).unwrap(),
                &box_::SecretKey::from_slice(sk).unwrap())
                .map_err(|_| Error::PlugCryptoFailure)?)
        };
        let m = Message::new();
        let id = m.headers.id;

        // Act and Assert
        let crypted =
            m.send_asymm(my_crypter,
            &nonce.as_ref(),
            &receiver_pk.as_ref(),
            &sender_sk.as_ref()
        );
        assert!(&crypted.is_ok()); // Encryption check
        let raw_m = Message::receive_asymm(
            &crypted.unwrap(),
            my_decrypter,
            &nonce.as_ref(),
            &sender_pk.as_ref(),
            &receiver_sk.as_ref());
        assert!(&raw_m.is_ok()); // Decryption check
        assert_eq!(id, raw_m.unwrap().headers.id);
    }

    #[test]
    #[allow(non_snake_case)]
    #[cfg(feature="raw-crypto")]
    fn plugin_crypto_xChaCha20Paly1305_x25519_dalek_shared_secret() {
        // Arrange
        let sender_sk = EphemeralSecret::new(OsRng);
        let sender_pk = PublicKey::from(&sender_sk);
        let receiver_sk = EphemeralSecret::new(OsRng);
        let receiver_pk = PublicKey::from(&receiver_sk);
        let sender_shared = sender_sk.diffie_hellman(&receiver_pk);
        let receiver_shared = receiver_sk.diffie_hellman(&sender_pk);
        let m = Message::new();
        let id = m.headers.id;
        // Plugable encryptor function to encrypt data
        let my_crypter = |k: &[u8], m: &[u8]| -> Vec<u8> {
            let aead = XChaCha20Poly1305::new(k.into());
            let nonce = XNonce::from_slice(b"extra long unique nonce!");
            aead.encrypt(nonce, m).expect("encryption failure!")
        };
        // Plugable decryptor function to decrypt data
        let my_decrypter = |k: &[u8], m: &[u8]|
            -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let aead = XChaCha20Poly1305::new(k.into());
            let nonce = XNonce::from_slice(b"extra long unique nonce!");
            Ok(aead.decrypt(nonce, m).unwrap())
        };

        // Act and Assert
        let crypted = m.send_raw(my_crypter, sender_shared.as_bytes());
        assert!(&crypted.is_ok()); // Encryption check
        let raw_m =
            Message::receive_raw(&crypted.unwrap(), my_decrypter, receiver_shared.as_bytes());
        assert!(&raw_m.is_ok()); // Decryption check
        assert_eq!(id, raw_m.unwrap().headers.id); // Data consistancy check
    }
}
