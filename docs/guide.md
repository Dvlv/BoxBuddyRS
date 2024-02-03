# BoxBuddy User Guide

## Creating a new Distrobox

Click the **+** button in the top-left of the window's header bar. This will bring up a new window where you can enter some options for your new box.

- **Name** - The name of your new box
- **Image** - The container image you wish to use for your new box. You can type the name of a distro to filter the dropdown list.
- **Use Init System** - Activate this switch to add systemd support in your box.
- **Home Directory** - If you wish for your box to have a separate home directory, click the file icon on the right-hand side of this row and choose a folder. **Note**: If you are using the Flatpak and do not see this option, it is because you need to allow Filesystem access to the sandbox. See [the documentation](/BoxBuddyRS/tips) for a guide.

Once these options are filled out, click the blue "Create" button in the top-right of the header bar to create your box. A loading spinner will appear while the box is being created, then a terminal window will spawn to begin initialising the box.

Bear in mind that if you do not have an image on your system when creating a new box, the image must first be pulled from the internet. This can sometimes take a minute or two, so please be patient.

### Using Distrobox Assemble
If you wish to use Distrobox's Assemble feature, click the wrench icon in the top-left, next to the **+** button. 

This will spawn a file-chooser window. Select your `.ini` file and press the blue "Open" button in the file-chooser's header bar.

A popup window will appear with a loading spinner, letting you know that your boxes are being created. Upon completion this window will disappear and your new boxes should appear in BoxBuddy. 

**Note** These boxes will need to be initialised before they can be used. Simply click the "Open Terminal" button against each box and wait for the initialisations to finish.

If no boxes appear, your `.ini` file may not be valid. Please check it and try again.

For more information on Distrobox Assemble, [check out the documentation here.](https://github.com/89luca89/distrobox/blob/main/docs/usage/distrobox-assemble.md)

## Using the Distrobox

### Opening a Terminal
To open a terminal in the box, click the "Open Terminal" button. This should spawn a terminal window running inside the box.

### Upgrading a Box
Click the "Upgrade Box" button to use the distro's package manager to upgrade all packages in the box. This will spawn a terminal window where you can watch the progress.

### Stopping a Box
If a box is running there will be a stop symbol in the top-right of the application window, next to the box's status (which will probably say "Up X Minutes"). Click this stop button to stop the box.

If the box is definitely running but the button does not appear, click the Reload button in the titlebar (The curved arrow) to reload BoxBuddy's UI. This should update the box's current status and make the stop button appear.

### Removing a Box
Click the "Delete Box" button to remove a box. A confirmation popup will appear to make sure you wish to permanently delete the box. Click "Delete" to confirm, or "Cancel" to go back.

### Managing Applications

#### Installing / Removing
There is currently no way to install or remove applications inside a box using BoxBuddy itself. To install packages, use the "Open Terminal" button to spawn a terminal inside your box, then use the distro's package manager to install and remove packages.

#### Running
Click the "View Applications" button to see a popup containing a list of all applications installed in the box. This may take a few seconds to load. 

Click the "Run" button next to your desired application to execute it.

#### Exporting / Unexporting
In the "View Applications" window you should see either an "Add To Menu" or "Remove From Menu" button next to each application. Clicking the "Add To Menu" button will export this application so that it appears in your system's menu. Likewise, clicking the "Remove From Menu" button will remove it from your menu.

