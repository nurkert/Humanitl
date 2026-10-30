# dmgbuild settings for the Humanitl disk image (HUM-316).
#
# dmgbuild writes the Finder layout (.DS_Store) itself, without remote
# controlling Finder, so the image looks the same when it is built on a CI
# runner. Called from packaging/macos/build-app.sh with
#   -D app=<Humanitl.app> -D background=<background.tiff> -D icon=<AppIcon.icns>
# Positions match the wells drawn in background.svg.
import os.path

application = defines["app"]  # noqa: F821 (injected by dmgbuild)
appname = os.path.basename(application)

format = "UDZO"
filesystem = "HFS+"
compression_level = 9

files = [application]
symlinks = {"Applications": "/Applications"}
hide_extension = [appname]

icon = defines["icon"]  # noqa: F821
background = defines["background"]  # noqa: F821

# Finder counts its title bar (28 pt) into the window, the background is 420.
window_rect = ((200, 140), (660, 448))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
sidebar_width = 0

arrange_by = None
grid_offset = (0, 0)
grid_spacing = 100
scroll_position = (0, 0)
label_pos = "bottom"
text_size = 13
icon_size = 112

icon_locations = {
    appname: (170, 196),
    "Applications": (490, 196),
}
