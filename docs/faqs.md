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
- elementary Terminal
- Ptyxis
- Foot
- Terminator
- XTerm
- COSMIC Terminal

## Can I use [some_terminal] instead?
Adding terminals is very simple, please open an Issue on GitHub and I will look into it.

NOTE: Blackbox terminal is currently unable to be supported.

## Can BoxBuddy launch Flatpak terminals?
BoxBuddy can launch a Flatpak of a terminal provided it is set as your Preferred Terminal. [Instructions are linked here.](https://www.dvlv.co.uk/BoxBuddyRS/guide#set-preferred-terminal)

## Why does a terminal sometimes close instantly when I open it?
The default behaviour of most terminal emulators (when spawned with a single command) is to close when the executed command completes. This means if something errors, the terminal emulator assumes it is finished, and exits. There is nothing BoxBuddy can do about this.

## How do I pass the --nvidia flag when creating a box?
BoxBuddy will detect whether you have NVIDIA hardware and pass this flag automatically!

## Can I enter a custom Image URL when creating a new box?
No, BoxBuddy will only allow you to create boxes from officially-supported images. You can use the Distrobox CLI to create a container with a custom image, and it will then show up in BoxBuddy as normal.

## Can you add [some_image] to the dropdown?
No, the dropdown is populated by the output of `distrobox create -C`, so please [file a request against Distrobox](https://github.com/89luca89/distrobox) to request new supported images.

## Why doesn't BoxBuddy package Distrobox?
As far as I know Podman needs to be on the host system to work properly. If anybody knows of another flatpak project which successfully re-packages Podman, feel free to open an issue with a link.
