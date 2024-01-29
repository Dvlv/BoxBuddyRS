# Tips

## Setting a Custom Home Directory with the Flatpak Version

The flatpak version of BoxBuddy is sandboxed, and cannot access the user's home directory unless granted permission. This can be done using the command line, or with an application called Flatseal

### Allowing Filesystem Access via Flatseal
Install Flatseal if you haven't already:

```bash
flatpak install flathub com.github.tchx84.Flatseal
```

Then open it up and click on BoxBuddy in the sidebar.
 to 
Scroll down to the **Filesystem** section and enable the switch labelled "All user files".

![Flatseal](flatseal-home-permissions.png)

Now BoxBuddy will have access to your home directory.

### Removing Filesystem Access via Flatseal

Open up Flatseal and select BoxBuddy in the sidebar.

Scroll down to the **Filesystem** section and disable the switch labelled "All user files".

Alternatively, you can click the "Reset" button in the application's titlebar to remove all custom permissions from BoxBuddy in one go.

### Allowing Filesystem Access via the Command Line
You will need to determine if BoxBuddy is a user-level or system-level flatpak.

To do this, execute:

```bash
flatpak list --columns=app,installation | grep boxbuddyrs
```

This should say either "user" or "system".

If you have BoxBuddy as a user-level flatpak, execute:

```bash
flatpak override --user io.github.dvlv.boxbuddyrs --filesystem=home
```

If BoxBuddy is instead a system-level flatpak, execute:

```bash
sudo flatpak override io.github.dvlv.boxbuddyrs --filesystem=home
```

### Removing Filesystem Access via the Command Line
After creating your Box with a custom home directory, you may wish to remove filesystem permissions again.



If you have BoxBuddy as a user-level flatpak, execute:

```bash
flatpak override --user --reset io.github.dvlv.boxbuddyrs 
```

If BoxBuddy is instead a system-level flatpak, execute:

```bash
sudo flatpak override --reset io.github.dvlv.boxbuddyrs
```

------
