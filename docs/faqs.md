# Frequently Asked Questions

## Why can't I see some of my existing Distroboxes?
Rootful distroboxes are not supported by BoxBuddy, as they are owned by the root user, and to support them would cause a deluge of password prompts.

## Why can't I see [some_feature] on the Flatpak version of BoxBuddy?
Some features require filesystem access, which is not granted by default. Please see [this tutorial](/BoxBuddyRS/tips) for a walkthrough of enabling or removing filesystem access.

## What terminals are supported?
BoxBuddy will try to spawn the following terminals, in the following order:

- Ptyxis
- GNOME Console
- GNOME Terminal
- Konsole
- Tilix
- Kitty
- Alacritty
- XTerm

## Can I use [some_terminal] instead?
Xfce-terminal cannot be supported due to the unusual way it needs to be spawned.

For any other terminal, please open an Issue and I will look into it.

## Why does a terminal sometimes close instantly when I open it?
The default behaviour of most terminal emulators (when spawned with a single command) is to close when the executed command completes. This means if something errors, the terminal emulator assumes it is finished, and exits. There is nothing BoxBuddy can do about this.

## Why doesn't BoxBuddy package Distrobox?
As far as I know Podman needs to be on the host system to work properly. If anybody knows of another flatpak project which successfully re-packages Podman, feel free to open an issue with a link.
