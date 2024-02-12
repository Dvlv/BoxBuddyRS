# Developer Documentation

## Setting up a development environment
To set up a development environment for BoxBuddy, you will need a Rust toolchain, make, and gettext.

You will also need Distrobox, as well as Podman (or docker).

### Using Distrobox Assemble
BoxBuddy's development environment can be spun up in a distrobox, using distrobox assemble. There is a file in the root of the project repo called `boxbuddy-dev-environment.ini` which can be passed to distrobox assemble like so:

```bash
distrobox assemble create --file boxbuddy-dev-environment.ini
```

Or using BoxBuddy itself, click the Hammer icon in the top-left of BoxBuddy's header bar and choose the `boxbuddy-dev-environment.ini` file from the file-picker.
Then open up the box by clicking the Open Terminal button to enter your development environment.

### Manual Setup
You need the GTK4 and Libadwaita development packages, flatpak development packages, as well as `make`.

**Fedora** / **openSUSE** - `gtk4-devel libadwaita-devel gettext-devel make flatpak-builder`

**Debian** / **Ubuntu** - `libgtk-4-dev libadwaita-1-dev gettext build-essential flatpak-builder`

**Arch** - `gtk4 libadwaita base-devel gettext flatpak-builder`

You will also need Rust, probably via [Rustup](https://rustup.rs/)

Install stable Rust via:

```bash
rustup toolchain add stable
rustup component add clippy
```

## Guidelines During Development
- Try to avoid adding external crates unless absolutely necessary
- Please run `make lint` before submitting changes
  - Please read and address any clippy warnings caused by your code additions, unless you can't.

- I would recommend having Flatseal installed while developing, so you can test your changes with and without host/home filesystem access easily.
  - `flatpak install flathub com.github.tchx84.Flatseal`

## Building and Running
Build and run the project by running `make`.

To test your changes as a flatpak, run `make flatpak` followed by `flatpak install --user boxbuddy.flatpak`.

## Updating the Potfile (Translation Framework)
If you add any translatable strings (calls to `gettext`) please run `make potfile` to update the potfile.
