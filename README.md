# ytmusic-tui — Omarchy edition

A keyboard-driven YouTube Music player with a practical desktop-friendly interface.

This is **lisim’s customised fork** of [WakaTaira/ytmusic-tui](https://github.com/WakaTaira/ytmusic-tui), based on upstream commit `09114f3076422756811c62889b605429df704813`. It is not an official upstream release. The original MIT licence and upstream history are preserved.

> **Read before signing in: this player uses your YouTube session cookies, not Google OAuth.** Browser sign-in reads your selected browser's local cookie store and saves a plaintext session copy. Manual header-paste also uses cookies. If you do not want to share a browser session with this app, do not authenticate. See [Authentication and privacy](#authentication-and-privacy).

![Home cards with sample content](screenshots/omarchy-home.png)

## What this edition adds

- **Home cards:** responsive two-column sections, longer titles and keyboard paging.
- **Visible shortcuts:** a contextual helper along the bottom; F1 hides/shows it.
- **Terminal colours:** choose `theme = "terminal"` to use your terminal palette.
- **Personal mixes:** later Home sections load, including Mixed for you. Personal mixes open promptly with their first batch of songs.
- **Desktop controls:** working MPRIS volume changes and upcoming-track metadata for the companion plugin.
- **Song ratings:** `f` toggles Like, `Shift+F` toggles Dislike; the helper shows both. The companion mini player can read/set/clear ratings through the same signed-in API.
- **Start from Play:** `ytmusic-tui --autoplay` plays the first available song from Home once recommendations load. Normal launches stay silent; keyboard input or a sign-in/API error cancels pending autoplay.

It also retains YouTube's rotating session timestamp cookies and supports opt-in renewal of expired sessions without restarting playback.

## Authentication and privacy

**This is unofficial browser-session authentication.** It is not a Google-approved OAuth consent flow, and the cookies are not a narrowly scoped, music-only access token. Treat them like a password: someone with a working copy can act as your signed-in YouTube session.

- **What is read:** `auth --from-browser chrome` (or another explicitly selected browser) uses `rookie` to read/decrypt that browser's local cookie store. Extraction requests YouTube domains; this fork retains an allowlist of cookie names only from `youtube.com` and its subdomains. It does not read your saved Google password. Do not run it unless you trust the app with that session.
- **Where it is stored/sent:** `browser.json` contains plaintext cookies and request headers. Files written by this fork use owner-only permissions (`0600`) on Unix and atomic replacement; they are not encrypted. The API client sends authentication to `https://music.youtube.com`. The project has no author-operated authentication server or cookie-upload/telemetry service. Never include this file or raw headers in issues, logs, screenshots or backups you share.
- **Automatic renewal is optional:** standalone launches do not read the browser store automatically. Setting `YTMUSIC_TUI_AUTH_BROWSER=chrome` explicitly enables renewal from that browser when a request is rejected or an account check shows a signed-out session. The newly extracted session must pass an account check before replacing the saved file; the failed request is retried once. Manual authentication alone does not enable renewal. Unset this variable to disable it and restart the player. The Omarchy plugin exposes this as a separate opt-in setting.
- **Manual does not mean cookie-free:** `ytmusic-tui auth` lets you paste YouTube request headers instead of allowing browser-store extraction. It still stores and uses the supplied cookies. Neither sign-in method is OAuth. Skipping authentication means account features such as your library, personalised mixes and ratings are unavailable.
- **Removing access:** quit the player and disable automatic renewal before deleting its `browser.json`. Default location: `~/.config/ytmusic-tui/browser.json`; `YTMUSIC_TUI_CONFIG_DIR` or an explicit `browser_auth_path` can change it. Removing the local copy does not revoke copies elsewhere; manage the underlying signed-in session through your Google account if necessary.

YouTube can expire sessions or change its unofficial interface. Renewal cannot recover a browser that is itself signed out, locked or inaccessible; the app reports the failure and preserves the previous auth file. An account check is used for ratings because watch metadata can also be returned anonymously.

## Use it with Omarchy

The [Omarchy YouTube Music plugin](https://github.com/lisim/omarchy-ytmusic) installs only this customised fork. It provides first-run setup, a floating show/hide window, playback controls, a vertical volume slider and an Up Next list. WakaTaira remains credited as the original TUI author; the upstream MIT licence is unchanged.

## Security improvements in this fork

Compared with our upstream base revision, this edition adds these specific protections:

- **Cookie domain boundaries:** the browser reader's substring search is not trusted. Before building headers or validating a session, we accept cookie rows only from `youtube.com` or a domain ending in `.youtube.com`, then apply the cookie-name allowlist. Lookalikes such as `youtube.com.example.invalid` and `notyoutube.com` are rejected. This check applies to initial browser import and automatic renewal.
- **Validated renewal:** replacements must pass a signed-in account check. Failure retains the saved session; an explicitly rejected request is retried only once. Rating reads also check sign-in because watch metadata can be returned anonymously.
- **Private atomic writes:** auth is written to an owner-only temporary file and atomically replaces the saved file after validation, avoiding partial writes. It remains plaintext, not encrypted storage.
- **No HTTP redirects for authenticated API calls:** session credentials are used against the fixed YouTube Music API endpoint; the client does not follow redirect responses.
- **Regression coverage:** synthetic unrelated-domain cookies are rejected before the validation callback and are absent from saved auth; valid YouTube cookies survive. The tests use dummy credentials and do not access a real browser account.

These are targeted improvements, not a security certification. Cookies still grant access to a signed-in session. Manual header-paste stores the supplied cookie header and cannot recover its per-cookie domain provenance; paste only headers from an actual `music.youtube.com` request. The companion plugin no longer offers the unpatched Original backend and requires fresh sign-in when migrating an old Original installation.

## Build it yourself

You need a stable Rust toolchain, a C toolchain/pkg-config, libmpv and yt-dlp. On Arch/Omarchy:

```sh
omarchy pkg add rustup base-devel mpv yt-dlp
rustup toolchain install stable
git clone --branch omarchy https://github.com/lisim/ytmusic-tui.git
cd ytmusic-tui
cargo build --release --locked -p ytmusic-tui
```

Run `target/release/ytmusic-tui auth --from-browser chrome` (replace `chrome` with your browser), then `target/release/ytmusic-tui`. Manual sign-in is available with `target/release/ytmusic-tui auth`.

By default it uses `~/.config/ytmusic-tui/`. Set **`YTMUSIC_TUI_CONFIG_DIR`** to a separate directory to isolate configuration, keymaps and browser authentication from another installation. The companion plugin does this automatically for this edition. Existing explicit `browser_auth_path` overrides remain supported.

For terminal colours, put this in the selected directory’s `config.toml`:

```toml
[ui]
theme = "terminal"
```

`NO_COLOR` disables terminal colours. The companion launcher clears it for this interactive player.

## Essential keys

| Key | Action |
| --- | --- |
| 1 / 2 / 3 / 5 / 8 | Home / Search / Library / Queue / Lyrics |
| Tab / Shift+Tab | Next / previous card or pane |
| Up / Down, Enter | Select and play/open |
| Space, n / p, + / - | Pause, next/previous, volume |
| f / Shift+F | Like / Dislike the current song; repeat to remove |
| Esc, F1, Q | Back or leave search input, helper, quit |

After submitting a search with Enter, press Esc to browse its results. Mouse selection is not supported. Closing the terminal quits playback; the companion plugin’s hide action keeps it running.

## Limits and validation

Home loads at most ten pages. Personal `RDTMAK` mixes load the initial batch (typically 100 songs); regular playlists retain full pagination. Desktop Up Next publishes at most 100 upcoming songs, respecting shuffle and repeat. Helper labels assume the default keymap.

```sh
cargo test --release --locked --workspace
cargo fmt --all --check
cargo clippy --release --all-targets --locked -- -D warnings
```

This repository distributes source, not prebuilt players. See [DEPENDENCIES.md](DEPENDENCIES.md) before redistributing binaries. [Upstream documentation](README_UPSTREAM.md) describes the original player; some behaviour differs in this edition.

## Licence and credit

[MIT](LICENSE), copyright © 2026 WakaTaira. Keep the original copyright and licence notice when redistributing. Fork changes are maintained by [lisim](https://github.com/lisim); this project is not affiliated with Google or YouTube.
