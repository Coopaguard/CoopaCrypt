# CoopaCrypt

An encrypted notebook for the things you must not lose and must not leak — passwords,
recovery codes, licence keys, private notes.

Your vault is **one plain Markdown document**, encrypted into a single self-contained
file. No account, no server, no sync service, no telemetry. Copy the file to a USB stick
or drop it in Nextcloud, Dropbox or a Git repository: it stays exactly as safe, because
nothing outside the file is needed to open it — and nothing outside the file can be
attacked to get in.

The editor shows **one chapter at a time**, navigated from a side panel built from your
Markdown headings. Long vaults stay readable instead of turning into an endless scroll.

---

## Download

**→ [Latest release](https://github.com/Coopaguard/CoopaCrypt/releases/latest)**

> **Status.** The application described here is the version 2 rewrite (Rust + Tauri),
> living on the `v2` branch. Anything released before `v0.2.0` is the original
> Windows-only WPF application, whose encryption is superseded — see
> [Version 1](#version-1-superseded).

| System | Architecture | File |
|---|---|---|
| Windows | x64 | `.msi` or `.exe` (NSIS installer) |
| Windows | ARM64 | `.exe` (NSIS — WiX produces no ARM64 MSI) |
| macOS | Apple Silicon, Intel | `.dmg` for your processor |
| Debian, Ubuntu, Mint | x64, ARM64 | `.deb` |
| Fedora, openSUSE | x64, ARM64 | `.rpm` |
| Arch, Manjaro | x64, ARM64 | `.AppImage`, or the [`PKGBUILD`](packaging/PKGBUILD) |
| NixOS | x64, ARM64 | [`flake.nix`](flake.nix) — `nix build github:Coopaguard/CoopaCrypt#app` |
| Any Linux, command line only | x64, ARM64 | `coopacrypt-cli-*-musl.tar.gz` — a single static binary |

### Verifying what you downloaded

The packages are not code-signed, so your operating system will not vouch for them.
Verify them yourself — for a tool that holds your passwords, this is worth the thirty
seconds.

Every release artifact carries a **build provenance attestation**, signed through
Sigstore using the release workflow's own OIDC identity. It proves the file came out of
this repository, at that commit, built by that workflow. Nothing was signed on anyone's
laptop:

```bash
gh attestation verify coopacrypt_2.1.0_x64-setup.exe -R Coopaguard/CoopaCrypt
```

Failing that, every release carries a `SHA256SUMS` file:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

When `SHA256SUMS.asc` is present, the checksum list is itself GPG-signed:

```bash
gpg --verify SHA256SUMS.asc SHA256SUMS
```

An attestation is not a code signature: Windows SmartScreen and macOS Gatekeeper ignore
it and will still warn about an unknown publisher. What it does give you is stronger than
what a certificate alone proves — a certificate says *someone* signed the file, whereas
an attestation says *which commit and which workflow* produced it. See
[Signing](#signing) for where code signing stands.

### Signing

Packages are **not code-signed**, on any platform. That is a cost problem rather than a
technical one, and it is worth stating plainly where it stands:

| Platform | Status |
|---|---|
| **Provenance attestation** | **Done.** Free, keyless, on every artifact. |
| **Windows** | Not yet. [SignPath Foundation](https://signpath.org/) grants free OV certificates to open-source projects and is the intended route. Note that an OV certificate does not clear SmartScreen on day one — reputation accrues with downloads. |
| **macOS** | Not yet. Notarization requires the Apple Developer Program at $99/year; there is no free path. |
| **Linux** | Not applicable. Direct-download packages are not expected to be signed; `SHA256SUMS` can carry a detached GPG signature. |

The release workflow is already wired for GPG: set the `GPG_PRIVATE_KEY` and
`GPG_PASSPHRASE` repository secrets and `SHA256SUMS.asc` appears in the next release. Without them
the step is skipped rather than failing.

### Updates

On every start the desktop application asks GitHub whether a newer release exists. If
so, it offers to download and install it — the way Notepad++ does. Declining just closes
the dialog; the offer comes back on the next start. If the check fails for any reason
(no network, GitHub unreachable), nothing is shown.

Update packages are signed with a [minisign](https://jedisct1.github.io/minisign/) key
whose public half is embedded in the application. A package whose signature does not
match is refused, so a compromised download location cannot push arbitrary code to
existing installs. The manifest the application reads is `latest.json`, attached to the
latest release.

Users who install through a package manager (`.deb`, `.rpm`, the Arch recipe, the Nix
flake) keep updating through it; the in-app update covers the Windows installers, the
macOS bundle and the Linux AppImage.

### About musl

The **command-line tool** is built against musl and is fully static: one file, no
system dependency, works on Debian, Arch, NixOS and Alpine alike.

The **desktop application cannot be**. Its interface is a native web view, which links
against webkit2gtk and therefore against glibc. There is no static musl build of the
GUI, and claiming otherwise would be dishonest. On distributions without a `.deb` or
`.rpm`, use the AppImage, the Arch recipe or the Nix flake.

---

## How your data is protected

The whole design follows one rule: **the file must be safe on its own**, wherever it
travels.

### File format

Every vault is laid out like this, and the layout is frozen and specified in
[`FORMAT.md`](FORMAT.md):

```text
64-byte header (plaintext, authenticated) │ ciphertext (N × 4096) │ 16-byte tag
```

| Element | Choice | Why |
|---|---|---|
| Key derivation | **Argon2id v0x13** — 128 MiB, 4 iterations, 4 lanes | Memory-hard: an attacker cannot trade cheap parallelism for speed |
| Salt | 16 random bytes, unique per file | No precomputed tables; cracking one vault never helps with another |
| Encryption | **XChaCha20-Poly1305** | Authenticated; its 192-bit nonce can be drawn at random with no collision risk |
| Nonce | 24 random bytes, new on every save | Never reused, even across thousands of saves |
| Integrity | Poly1305 tag over ciphertext **and header** | Tampering with the file — including weakening the stored Argon2id parameters — makes decryption fail |
| Padding | Content padded to a multiple of 4 KiB | A sync provider sees the file size; padding hides how much you actually wrote |

### What that buys you

**Brute force is expensive.** The old version hashed your password once with SHA-256 —
roughly 10⁹ guesses per second on a good GPU. Argon2id at 128 MiB brings that down to
around 10² per second. That factor of ten million is where the real security gain lives,
far more than the choice of cipher.

**Tampering is detected.** Change a single byte anywhere in the file and opening it fails
loudly. There is no silent corruption and no padding-oracle attack surface.

**Nothing leaks through metadata.** Two vaults of very different length produce files of
identical size. The header is readable without a password, but it only contains algorithm
parameters — never anything derived from your content.

**Cross-platform passphrases work.** Passwords are normalised to Unicode NFC before
derivation (RFC 8265), so an accented passphrase typed on macOS opens a vault created on
Windows. Normalisation applies to the **password only** — your content is stored byte for
byte, accents, emoji and line endings included. That matters: a vault stores passwords,
and silently altering one would break the account it unlocks.

**Saving cannot corrupt the vault.** Writes go to a temporary file, are flushed to disk,
then atomically replace the original. A crash or power loss mid-save leaves you with the
previous version, never a truncated one. There is no backup copy, so the application also
refuses to overwrite an existing vault by accident.

**Parameters harden themselves.** Every save re-derives with the current version's
defaults, so an old vault is silently upgraded the first time you save it.

### In the application

- Your password lives in **Rust memory only**, wiped on lock. It is never handed to the
  JavaScript side, so it is out of reach of the DOM and any frontend dependency.
- The session **auto-locks after 10 minutes** of inactivity. The deadline is enforced in
  Rust, not by a browser timer that a suspended window might never fire.
- Locking clears the editor, the preview **and the chapter list** — an outline of your
  chapter titles is already a disclosure.
- The window title never reflects your content, so nothing leaks into the taskbar or a
  screenshot.
- The Markdown preview is sanitised (DOMPurify) under a strict Content Security Policy,
  and links are inert. Rendered HTML runs inside the app's own webview, so a script
  smuggled into a document would otherwise run with the application's privileges.
- The application makes **no network requests at all**.

### What it does not protect against

Being explicit matters more than sounding reassuring:

- **A weak passphrase.** Encryption cannot compensate for a guessable password. Against a
  well-funded attacker, three random words fall in days; five put you out of reach. Aim
  for five words drawn at random — not chosen by you.
- **A compromised machine.** Malware, a keylogger or a memory dump while the vault is
  unlocked defeats any file format.
- **Forgetting your password.** There is no recovery, no hint, no reset. The file is
  mathematically unopenable without it.
- **Losing the file.** There is no cloud copy. Keep backups.

---

## Building from source

### Prerequisites

| | |
|---|---|
| **Rust** | stable, 1.77 or newer ([rustup](https://rustup.rs)) |
| **Node.js** | 20 or newer, with npm |
| **Windows** | WebView2 runtime (preinstalled on Windows 11) and MSVC build tools |
| **macOS** | Xcode command line tools |
| **Linux** | `webkit2gtk-4.1`, `libayatana-appindicator3`, `librsvg2` — see the [Tauri prerequisites](https://tauri.app/start/prerequisites/) |

### Layout

```text
crates/coopacrypt-core/   Reference implementation of the file format. No disk I/O,
                          no UI — pure buffers in, buffers out. This is the source of
                          truth; its frozen test vectors are the conformance contract.
crates/coopacrypt-cli/    Command-line tool. Also the format's test bench.
crates/coopacrypt-app/    Tauri backend: session, commands, atomic writes.
app/                      Frontend: Vite, TypeScript, CodeMirror 6.
```

### Run the desktop app

```bash
npm --prefix app install
npm --prefix app run tauri dev
```

`tauri dev` starts the Vite dev server and the Rust backend together, with hot reload on
the frontend.

> Running `cargo run -p coopacrypt-app` on its own in debug will show
> `ERR_CONNECTION_REFUSED`: a debug build loads the frontend from the dev server rather
> than from the bundle. Use `tauri dev`, or build in release.

### Build a release binary

```bash
npm --prefix app install
npm --prefix app run tauri build
```

The standalone executable lands in `target/release/`, and the platform installers in
`target/release/bundle/`.

### Use the CLI

```bash
cargo run -p coopacrypt-cli -- --help

cargo run -p coopacrypt-cli -- encrypt notes.md -o vault.coocrypt
cargo run -p coopacrypt-cli -- info vault.coocrypt      # header, no password needed
cargo run -p coopacrypt-cli -- decrypt vault.coocrypt
cargo run -p coopacrypt-cli -- chpass vault.coocrypt
```

Passwords are read from the terminal with echo disabled. If stdin is redirected, a line
is read from it instead — convenient for scripting and CI, but never do that with a real
vault: the password can end up in shell history or a CI log.

### Tests

```bash
cargo test                          # 53 tests: format, header, session, atomic writes, launch
cargo test --release -- --ignored   #  4 CLI integration tests (real Argon2id, slow)
npm --prefix app test               # 66 tests: chapter splitting, search, live preview
npm --prefix app run build          # strict type-check, then bundle
cargo clippy --all-targets
cargo fmt --all --check
```

The CLI integration tests are `#[ignore]`d by default because each one performs real
Argon2id derivations at production parameters. **No switch to weaken the KDF is exposed**,
not even for tests: a security tool has no business shipping an "encrypt less well"
button.

### Continuous integration and releases

| Workflow | Trigger | What it does |
|---|---|---|
| `.github/workflows/ci.yml` | every pull request, and every push to `master` | format, clippy (warnings are errors), Rust tests, CLI integration tests, TypeScript type-check, frontend tests, compile check on Linux/Windows/macOS, and a **frozen-test-vector guard** |
| `.github/workflows/release.yml` | a pull request from a `v<major>.<minor>.<patch>` branch merged into `master` | tags the merge commit `v<version>`, builds every package listed under [Download](#download), generates `SHA256SUMS`, then publishes |

Cutting a release:

1. Create a branch named after the version, e.g. `v2.1.0`.
2. Set that version in `Cargo.toml`, `Cargo.lock`, `app/package.json`,
   `app/package-lock.json`, `crates/coopacrypt-app/tauri.conf.json`, `flake.nix`
   and `packaging/PKGBUILD`.
3. Open a pull request and merge it.

The workflow checks that the branch name matches the version in `tauri.conf.json`,
tags the merge commit `v<version>`, creates a draft release, fills it in from every
build job, and only publishes once all of them have succeeded.

Merging any other branch (`feature/…`, `fix/…`) publishes nothing. Closing a pull
request without merging publishes nothing either.

To retry a failed publication, run the workflow by hand from the Actions tab with the
existing tag.

The workflow **requires** the `TAURI_SIGNING_PRIVATE_KEY` repository secret (and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key has one): it signs the update packages
described under [Updates](#updates), and `prepare` fails early without it. The key is
generated once with `npx tauri signer generate -w <path>` from `app/`; the public half
goes in `plugins.updater.pubkey` of `tauri.conf.json`, the private half in the secret
and nowhere else. Losing it means every installed copy stops seeing updates — there is
no way to rotate it remotely.

The frozen-vector guard deserves a word: CI regenerates `vectors.json` and fails if it
differs from what is committed. A change to the file format, or merely to the default
Argon2id parameters, can therefore no longer slip through unnoticed — it has to be an
explicit commit.

### Changing the file format

Don't, unless you mean to. `crates/coopacrypt-core/tests/vectors.json` holds 13 frozen
vectors that any implementation must reproduce byte for byte. If a conformance test fails,
find what moved in the code — regenerating the vectors to make the failure disappear
silently changes the format and orphans existing vaults.

---

## Documentation

| | |
|---|---|
| [`FORMAT.md`](FORMAT.md) | File format specification — frozen, with test vectors |

## Licence

MIT — see [`LICENSE.txt`](LICENSE.txt).
