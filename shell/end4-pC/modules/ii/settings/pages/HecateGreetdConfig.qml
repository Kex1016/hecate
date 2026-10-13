import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets
import qs.services

/**
 * hecate-greetd, the login screen. It is this lock screen, run by greetd inside hecate before
 * anyone logs in, with the look of whoever is logging in: wallpaper, colours, clock and widgets,
 * monitor layout, keyboard layout. These settings live in the shell config like the rest and
 * reach the login screen through `hecate-greetd sync` (GreeterSync).
 */
ContentPage {
    id: page
    forceWidth: true

    readonly property var greeter: Config.options.greeter

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 20

        NoticeBox {
            Layout.fillWidth: true
            visible: !GreeterSync.available
            materialIcon: "info"
            text: Translation.tr("hecate-greetd isn't installed. Turn it on in your NixOS config with services.hecate-greetd.enable = true; these settings apply once it is.")
        }

        NoticeBox {
            Layout.fillWidth: true
            visible: GreeterSync.available && GreeterSync.error !== ""
            materialIcon: "error"
            color: Appearance.colors.colErrorContainer
            text: GreeterSync.error
        }

        ContentSection {
            icon: "login"
            title: Translation.tr("Login screen")

            StyledText {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Appearance.colors.colSubtext
                text: Translation.tr("The login screen is this shell's lock screen. When you pick your account it shows your wallpaper, colours, clock and widgets on your monitor layout, and plays the unlock animation into your session.")
            }

            ConfigSwitch {
                buttonIcon: "share"
                text: Translation.tr("Show my look on the login screen")
                checked: page.greeter.share
                onCheckedChanged: page.greeter.share = checked
            }

            ConfigComboBox {
                Layout.fillWidth: true
                enabled: page.greeter.share
                buttonIcon: "wallpaper"
                text: Translation.tr("Wallpaper style")
                fieldWidth: 280
                fixedWidth: true
                model: [
                    { displayName: Translation.tr("Same as the lock screen"), value: "lock" },
                    { displayName: Translation.tr("Centered in a shape"), value: "centered" },
                    { displayName: Translation.tr("Blurred"), value: "blurred" },
                ]
                currentValue: page.greeter.style
                onSelected: newValue => page.greeter.style = newValue
            }

            RowLayout {
                id: wallpaperRow
                Layout.fillWidth: true
                Layout.leftMargin: 8
                enabled: page.greeter.share
                spacing: 10

                MaterialSymbol {
                    text: "image"
                    iconSize: Appearance.font.pixelSize.larger
                    color: Appearance.colors.colOnSecondaryContainer
                }
                StyledText {
                    Layout.fillWidth: true
                    elide: Text.ElideMiddle
                    color: Appearance.colors.colOnSecondaryContainer
                    text: page.greeter.wallpaper !== ""
                        ? page.greeter.wallpaper
                        : (Config.options.background.lockWall !== ""
                            ? Translation.tr("Wallpaper: the lock screen's")
                            : Translation.tr("Wallpaper: the desktop's"))
                }
                RippleButtonWithIcon {
                    buttonRadius: Appearance.rounding.small
                    materialIcon: "folder_open"
                    mainText: Translation.tr("Choose…")
                    enabled: !pickProc.running
                    onClicked: pickProc.running = true
                }
                RippleButtonWithIcon {
                    visible: page.greeter.wallpaper !== ""
                    buttonRadius: Appearance.rounding.small
                    materialIcon: "close"
                    mainText: Translation.tr("Clear")
                    onClicked: page.greeter.wallpaper = ""
                }

                Process {
                    id: pickProc
                    command: ["bash", "-c",
                        'if command -v kdialog >/dev/null 2>&1; then kdialog --title "$1" --getopenfilename "$2" "Images (*.png *.jpg *.jpeg *.webp *.avif *.bmp *.gif *.tif *.tiff)"; else zenity --file-selection --title="$1" --filename="$2/" --file-filter="Images | *.png *.jpg *.jpeg *.webp *.avif *.bmp *.gif *.tif *.tiff"; fi',
                        "pick", Translation.tr("Choose the login screen's wallpaper"),
                        page.greeter.wallpaper !== "" ? page.greeter.wallpaper.replace(/\/[^/]*$/, "") : `${Quickshell.env("HOME")}/Pictures`]
                    stdout: StdioCollector {
                        onStreamFinished: {
                            const path = text.trim();
                            if (path !== "")
                                page.greeter.wallpaper = path;
                        }
                    }
                }
            }

            ConfigSwitch {
                enabled: page.greeter.share
                buttonIcon: "toast"
                text: Translation.tr("Show the toolbars beside the password field")
                checked: page.greeter.showToolbars
                onCheckedChanged: page.greeter.showToolbars = checked
            }

            ConfigSwitch {
                enabled: page.greeter.share
                buttonIcon: "lock"
                text: Translation.tr("Show a status line under the clock")
                checked: page.greeter.showStatusText
                onCheckedChanged: page.greeter.showStatusText = checked
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 8
                visible: page.greeter.showStatusText
                enabled: page.greeter.share
                spacing: 10
                MaterialSymbol {
                    text: "text_fields"
                    iconSize: Appearance.font.pixelSize.larger
                    color: Appearance.colors.colOnSecondaryContainer
                }
                MaterialTextField {
                    Layout.fillWidth: true
                    placeholderText: Translation.tr("Locked")
                    text: page.greeter.statusText
                    onEditingFinished: page.greeter.statusText = text
                }
            }
        }

        ContentSection {
            icon: "monitor"
            title: Translation.tr("Displays and input")

            StyledText {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Appearance.colors.colSubtext
                text: Translation.tr("Your monitors (resolution, refresh rate, scale, orientation and position, from your Nix config and the hecate page) and keyboard layout apply as soon as your account is picked, including monitors that are plugged in later.")
            }
            ConfigSwitch {
                enabled: page.greeter.share
                buttonIcon: "desktop_windows"
                text: Translation.tr("Use my monitor layout")
                checked: page.greeter.displays
                onCheckedChanged: page.greeter.displays = checked
            }
            ConfigSwitch {
                enabled: page.greeter.share
                buttonIcon: "keyboard"
                text: Translation.tr("Use my keyboard layout, mouse and touchpad settings")
                checked: page.greeter.input
                onCheckedChanged: page.greeter.input = checked
            }
        }

        ContentSection {
            icon: "sync"
            title: Translation.tr("Sync")

            StyledText {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: Appearance.colors.colSubtext
                text: {
                    if (!GreeterSync.available) return Translation.tr("Not installed.");
                    if (GreeterSync.syncing) return Translation.tr("Syncing…");
                    if (!page.greeter.share) return Translation.tr("Not shared: the login screen shows its default look for you.");
                    const s = GreeterSync.status;
                    if (!s.synced_at) return Translation.tr("Not synced yet.");
                    const when = new Date(s.synced_at * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat);
                    const outputs = (s.outputs ?? []).join(", ");
                    return Translation.tr("Last synced %1. %2 files, displays: %3.")
                        .arg(when).arg(s.files ?? 0).arg(outputs || Translation.tr("none"));
                }
            }

            RowLayout {
                Layout.alignment: Qt.AlignRight
                RippleButtonWithIcon {
                    buttonRadius: Appearance.rounding.small
                    materialIcon: "visibility"
                    mainText: Translation.tr("Preview")
                    enabled: GreeterSync.available && !GreeterSync.syncing
                    onClicked: GreeterSync.preview()
                }
                RippleButtonWithIcon {
                    buttonRadius: Appearance.rounding.small
                    materialIcon: "sync"
                    mainText: Translation.tr("Sync now")
                    enabled: GreeterSync.available && !GreeterSync.syncing
                    onClicked: GreeterSync.syncNow()
                }
            }
        }
    }
}
