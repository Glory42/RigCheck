# Signed manifest fixture

`manifest.json` as `pipeline publish` writes it, and `manifest.sig`: its
Ed25519 signature in hex, made the way the Build games.db workflow makes it:

```sh
openssl pkeyutl -sign -rawin -inkey key.pem -in manifest.json | od -An -tx1 -v | tr -d ' \n'
```

The key is a throwaway made for this test, not the release key. Its public
key (raw, hex) is `59d9af65a316d50d4b3df8adb6a2712d004ec76514c24111e2da1dc5969c982d`, which the tests in
`crates/gamedb/tests/update.rs` verify against.
