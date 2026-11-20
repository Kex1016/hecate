import QtQuick
import qs
import qs.services
import qs.modules.ii.settings

SettingsPage {
    id: page

    readonly property var status: Updates.cakeosStatus

    Component.onCompleted: SystemInfo.refresh()

    SetColumn {
        SetSection { text: "this machine" }
        SetInfo { name: "host"; text: SystemInfo.hostname }
        SetInfo { name: "os"; text: SystemInfo.distroName }
        SetInfo { name: "kernel"; text: SystemInfo.kernelVersion }
        SetInfo { name: "cpu"; text: SystemInfo.cpu }
        SetInfo { name: "gpu"; text: SystemInfo.gpu }
        SetInfo { name: "memory"; text: SystemInfo.memory }
        SetInfo { name: "packages"; text: SystemInfo.packages }
        SetInfo { name: "installed"; text: SystemInfo.installAge }
    }
    SetColumn {
        SetSection { text: "cakeos" }
        SetInfo { name: "branch"; text: page.status?.branch ?? "" }
        SetInfo { name: "installed"; text: (page.status?.installed ?? "").slice(0, 7) }
        SetAction {
            name: "updates"
            description: !Updates.cakeos ? "This is not a CakeOS install."
                : page.status?.behind > 0 ? `${page.status.behind} new changes upstream.`
                : "Up to date."
            buttons: [{ text: page.status?.behind > 0 ? "open updater" : "check", ink: page.status?.behind > 0 ? "c3" : "c1" }]
            enabled: Updates.cakeos
            onPressed: {
                GlobalStates.settingsOpen = false;
                Updates.openUpdater();
            }
        }
        SetSection { text: "shell" }
        SetNote {
            narrator: true
            text: "You are running hecate's shell, dressed in haiiro: Rosé Pine, the IBM VGA face and square corners. There is no theme page, because there is only one theme."
        }
    }
}
