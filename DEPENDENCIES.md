# Dependency licensing

The source of this fork retains upstream's [MIT licence](LICENSE). This repository does not distribute compiled executables or vendored third-party dependencies.

Cargo dependencies are resolved from `Cargo.lock` and keep their own licences. Runtime/build dependencies include libmpv, yt-dlp and the relevant system audio/media libraries. Installing these from the operating system's repositories does not relicense them as MIT.

In particular, [mpv's copyright document](https://github.com/mpv-player/mpv/blob/master/Copyright) describes its default GPL build and conditional LGPL build; linked libraries such as FFmpeg can change the applicable binary-distribution obligations. Before distributing a prebuilt player, review the exact build and provide all required licences, notices and source or source offers. Do not describe a combined executable as MIT-only based on the root licence.

The [companion Omarchy plugin](https://github.com/lisim/omarchy-ytmusic) has its own MIT notice, including credit for Omarchy code. It builds a pinned revision of this player locally rather than shipping a prebuilt executable.
