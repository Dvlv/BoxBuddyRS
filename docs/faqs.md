# Frequently Asked Questions

## Why can't I see some of my existing Distroboxes?
Rootful distroboxes are not supported by BoxBuddy, as they are owned by the root user, and to support them would cause a deluge of password prompts.

## Why can't I see [some_feature] on the Flatpak version of BoxBuddy?
Some features require filesystem access, which is not granted by default. Please see [this tutorial](/BoxBuddyRS/tips) for a walkthrough of enabling or removing filesystem access.

## What terminals are supported?
If your preferred terminal is set via the menu, BoxBuddy will first try and spawn that one. If unset, or your preference cannot be found, BoxBuddy will then check for the first supported terminal in the priority list.

Terminals are searched in the following order:

- GNOME Console
- GNOME Terminal
- Konsole
- Xfce4 Terminal
- Tilix
- Kitty
- Alacritty
- WezTerm
- Ptyxis
- Foot
- XTerm
- COSMIC Terminal

## Can I use [some_terminal] instead?
Adding terminals is very simple, please open an Issue on GitHub and I will look into it.

## My preferred terminal is a Flatpak, why can't BoxBuddy open it?
BoxBuddy is not programmed to be able to launch Flatpaks.

You can get around this by making a wrapper script which is named after the executable of your terminal. As long as this wrapper script is in your PATH it should be picked up by BoxBuddy as if it were the actual program.

Here is an example script for Wezterm:

```bash
#!/usr/bin/env bash
flatpak run org.wezfurlong.wezterm $@
```

Save this script as `wezterm` and place it somewhere in your path (for example, `~/.local/bin`) and BoxBuddy can then launch the Flatpak of Wezterm.

## Why does a terminal sometimes close instantly when I open it?
The default behaviour of most terminal emulators (when spawned with a single command) is to close when the executed command completes. This means if something errors, the terminal emulator assumes it is finished, and exits. There is nothing BoxBuddy can do about this.

## How do I pass the --nvidia flag when creating a box?
BoxBuddy will detect whether you have NVIDIA hardware and pass this flag automatically!

## Why doesn't BoxBuddy package Distrobox?
As far as I know Podman needs to be on the host system to work properly. If anybody knows of another flatpak project which successfully re-packages Podman, feel free to open an issue with a link.
