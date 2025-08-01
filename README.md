# Quick Launch
An application that allows you to launch applications, and run shell scripts on your computer by pressing a predefined series of keys.

# Usage 
--config CONFIG - the config file that you want to use (see config.json for an example).

--css CSS - the stylesheet that you want to use see src/style.css for an example

# Configuration
**width**: positive integer -- how many columns should the launcher have

**height**: positive integer -- not used

**icon_size**: how big do you want to make the icons for the applications

**applications**: a map from a string representing the gdk keycode to either an application or folder.

## Application Options
**command** the command used to launch the application (overides launch command from **application** option)

**image** image to display for the application (overides the image from **application** option) (can not be used with **icon**)

**icon** gtk icon to display for the application (overides the image from the **application** option) (can not be used with **image**)

**name** name to display under the application (overides the name from **application** option)

**application** application id, provides defaults for command image/icon and name, if you don't use this you'll need to specify these manually. 

## Folder Options
**name** name of the folder to display under the folder.

**icon** gtk icon to use for the folder (can not be used with **image**)

**image** image to use for the folder (can not be used with **icon**)

**applications** a map from a string representing the gdk keycode to either an application or folder **required**

## Installation
just run the following command

```
git clone https://github.com/astaugaard/quick-launch.git; cargo install --path quick-launch
```
