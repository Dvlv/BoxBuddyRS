# Contributing

Thank you for considering contributing to BoxBuddy!

## Developing

BoxBuddy is built with Rust, and uses a standard `cargo` workflow.

### Dependencies

You need the GTK4 and Libadwaita development packages.

**Fedora** / **openSUSE** - `gtk4-devel libadwaita-devel`

**Debian** - `libgtk-4-dev libadwaita-1-dev`

**Arch** - `gtk4 libadwaita`

You may also need `make`.

You will  need Rust, probably via [Rustup](https://rustup.rs/)

Install stable Rust via:

```bash
rustup toolchain add stable
```

### Building / Running

```bash
cargo run
```

Or just

```bash
make
```

### Building Flatpak

- Install `flatpak-builder`
- Execute `make flatpak`
- Install the bundle: `flatpak install --user boxbuddy.flatpak`

### Coding Guidelines

Please run `cargo fmt` and `cargo clippy` before submitting code. This can be done with `make lint`.

Try to avoid adding external crates unless absolutely necessary.

## Translations

Translations are much appreciated. There are instructions in the [po folder.](https://github.com/Dvlv/BoxBuddyRS/tree/master/po)

## Issues / Feature Requests

Feature requests are welcome! Please check the [roadmap](https://github.com/Dvlv/BoxBuddyRS/blob/master/docs/ROADMAP.md) to see if a feature is already planned.

When filing issues, please keep in mind that BoxBuddy is just a GUI, and I am not a developer of Distrobox itself. Any issues with created boxes are probably better logged [upstream.](https://github.com/89luca89/distrobox/issues)
