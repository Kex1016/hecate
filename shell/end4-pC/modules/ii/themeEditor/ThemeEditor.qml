pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import org.kde.syntaxhighlighting
import qs
import qs.services
import qs.modules.common
import qs.modules.common.functions
import qs.modules.common.widgets

/**
 * hecate: the app theming editor. Pick a template (or a file to theme), edit it with the
 * colours a click away, preview it with the current colours, save. The templates themselves
 * are scripts/colors/hecate_templates.py's (--list, --preview, --save).
 */
Scope {
    id: root
    readonly property string script: Quickshell.shellPath("scripts/colors/hecate_templates.py")

    IpcHandler {
        target: "themeEditor"
        function open(): void {
            GlobalStates.themeEditorOpen = true;
        }
    }

    Loader {
        active: GlobalStates.themeEditorOpen
        sourceComponent: PanelWindow {
            id: panel
            screen: Quickshell.screens.find(s => s.name === WM.focusedMonitor?.name) ?? Quickshell.screens[0]
            exclusionMode: ExclusionMode.Ignore
            WlrLayershell.namespace: "quickshell:themeEditor"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            color: "transparent"
            anchors {
                top: true
                bottom: true
                left: true
                right: true
            }

            // pick -> edit -> preview
            property string step: "pick"
            property var templates: ({})
            property var colors: ({})
            property string templateId: ""
            property string output: ""
            property string format: "hex"
            property string preview: ""
            property string error: ""
            readonly property bool isNew: !(panel.templateId in panel.templates)
            readonly property bool validId: /^[A-Za-z0-9_-]+$/.test(panel.templateId)

            function close() {
                GlobalStates.themeEditorOpen = false;
            }
            // Edit an existing template, or start a new one from the target file as it is.
            function startEditing() {
                panel.error = "";
                source.path = "";
                source.path = panel.isNew ? panel.output : panel.templates[panel.templateId].input;
                panel.step = "edit";
            }

            Process {
                running: true
                command: ["python3", root.script, "--list"]
                stdout: StdioCollector {
                    onStreamFinished: {
                        try {
                            const data = JSON.parse(text);
                            panel.templates = data.templates;
                            panel.colors = data.colors;
                        } catch (e) {}
                    }
                }
                stderr: StdioCollector {
                    onStreamFinished: if (text.trim() !== "") panel.error = text.trim()
                }
            }
            FileView {
                id: source
                onLoaded: editor.text = source.text()
                onLoadFailed: editor.text = ""
            }
            Process {
                id: pickProc
                command: ["bash", "-c",
                    'if command -v kdialog >/dev/null 2>&1; then kdialog --title "$1" --getsavefilename "$2"; else zenity --file-selection --save --title="$1" --filename="$2/"; fi',
                    "pick", Translation.tr("File to theme"), `${Quickshell.env("HOME")}/.config`]
                stdout: StdioCollector {
                    onStreamFinished: {
                        const path = text.trim();
                        if (path !== "") panel.output = path;
                    }
                }
            }
            Process {
                id: previewProc
                stdinEnabled: true
                command: ["python3", root.script, "--preview"]
                onStarted: {
                    previewProc.write(editor.text);
                    previewProc.stdinEnabled = false;
                }
                onExited: previewProc.stdinEnabled = true
                stdout: StdioCollector {
                    onStreamFinished: panel.preview = text
                }
                stderr: StdioCollector {
                    onStreamFinished: panel.error = text.trim()
                }
            }
            Process {
                id: saveProc
                stdinEnabled: true
                command: ["python3", root.script, "--save", panel.templateId, panel.output]
                onStarted: {
                    saveProc.write(editor.text);
                    saveProc.stdinEnabled = false;
                }
                onExited: (exitCode, exitStatus) => {
                    saveProc.stdinEnabled = true;
                    if (exitCode === 0) panel.close();
                }
                stderr: StdioCollector {
                    onStreamFinished: if (text.trim() !== "") panel.error = text.trim()
                }
            }

            WindowDialog {
                anchors.fill: parent
                backgroundWidth: 900
                show: false
                focus: true
                Component.onCompleted: show = true
                onDismiss: panel.close()

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    MaterialSymbol {
                        iconSize: 26
                        text: "format_paint"
                        color: Appearance.colors.colSecondary
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        WindowDialogTitle {
                            Layout.fillWidth: true
                            text: Translation.tr("App theming")
                        }
                        StyledText {
                            Layout.fillWidth: true
                            elide: Text.ElideMiddle
                            color: Appearance.colors.colSubtext
                            text: panel.step === "pick" ? Translation.tr("Choose a template, or a file to theme")
                                : panel.step === "edit" ? Translation.tr("%1 → %2 · click a colour to insert it").arg(panel.templateId).arg(panel.output)
                                : Translation.tr("Preview with the current colours")
                        }
                    }
                }

                // Fixed height: WindowDialog sizes its card once, when it opens.
                Item {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 460

                    // 1. Which template.
                    ColumnLayout {
                        anchors.fill: parent
                        visible: panel.step === "pick"
                        spacing: 10

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Appearance.rounding.normal
                            color: Appearance.colors.colLayer2
                            clip: true
                            StyledListView {
                                id: templateList
                                anchors.fill: parent
                                anchors.margins: 8
                                model: Object.keys(panel.templates)
                                delegate: DialogListItem {
                                    id: item
                                    required property string modelData
                                    width: templateList.width
                                    active: panel.templateId === item.modelData
                                    onClicked: {
                                        panel.templateId = item.modelData;
                                        panel.output = panel.templates[item.modelData].output;
                                        panel.startEditing();
                                    }
                                    contentItem: RowLayout {
                                        StyledText {
                                            color: Appearance.colors.colOnLayer2
                                            text: item.modelData
                                        }
                                        StyledText {
                                            Layout.fillWidth: true
                                            horizontalAlignment: Text.AlignRight
                                            elide: Text.ElideMiddle
                                            color: Appearance.colors.colSubtext
                                            text: panel.templates[item.modelData].output
                                        }
                                    }
                                }
                            }
                            StyledText {
                                anchors.centerIn: parent
                                visible: templateList.count === 0
                                color: Appearance.colors.colSubtext
                                text: Translation.tr("No templates yet: start one below")
                            }
                        }

                        WindowDialogSectionHeader {
                            text: Translation.tr("New template")
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10
                            MaterialTextField {
                                id: idField
                                Layout.preferredWidth: 220
                                placeholderText: Translation.tr("Name (letters, -, _)")
                                onTextChanged: panel.templateId = text
                            }
                            StyledText {
                                Layout.fillWidth: true
                                elide: Text.ElideMiddle
                                color: panel.output !== "" ? Appearance.colors.colOnLayer1 : Appearance.colors.colSubtext
                                text: panel.output !== "" ? panel.output : Translation.tr("No file chosen")
                            }
                            RippleButtonWithIcon {
                                buttonRadius: Appearance.rounding.small
                                materialIcon: "folder_open"
                                mainText: Translation.tr("File to theme…")
                                enabled: !pickProc.running
                                onClicked: pickProc.running = true
                            }
                        }
                    }

                    // 2. The editor, with the colours beside it.
                    RowLayout {
                        anchors.fill: parent
                        visible: panel.step === "edit"
                        spacing: 10

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            radius: Appearance.rounding.normal
                            color: Appearance.colors.colLayer2
                            clip: true
                            Flickable {
                                id: editorFlick
                                anchors.fill: parent
                                anchors.margins: 8
                                contentWidth: editor.contentWidth
                                contentHeight: editor.contentHeight
                                clip: true
                                StyledTextArea {
                                    id: editor
                                    width: Math.max(editorFlick.width, implicitWidth)
                                    height: Math.max(editorFlick.height, implicitHeight)
                                    wrapMode: TextEdit.NoWrap
                                    font.family: Appearance.font.family.monospace
                                    background: null
                                    focus: panel.step === "edit"
                                    SyntaxHighlighter {
                                        textEdit: editor
                                        repository: Repository
                                        definition: Repository.definitionForFileName(panel.output)
                                        theme: Appearance.syntaxHighlightingTheme
                                    }
                                }
                            }
                        }

                        ColumnLayout {
                            Layout.preferredWidth: 260
                            Layout.fillHeight: true
                            spacing: 8
                            // M3 Expressive connected button group.
                            Row {
                                spacing: 2
                                Repeater {
                                    model: ["hex", "hex_stripped", "rgb"]
                                    SelectionGroupButton {
                                        required property string modelData
                                        required property int index
                                        leftmost: index === 0
                                        rightmost: index === 2
                                        buttonText: modelData
                                        toggled: panel.format === modelData
                                        onClicked: panel.format = modelData
                                    }
                                }
                            }
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                radius: Appearance.rounding.normal
                                color: Appearance.colors.colLayer2
                                clip: true
                                StyledListView {
                                    id: colorList
                                    anchors.fill: parent
                                    anchors.margins: 6
                                    model: Object.keys(panel.colors)
                                    delegate: DialogListItem {
                                        id: colorItem
                                        required property string modelData
                                        width: colorList.width
                                        verticalPadding: 6
                                        onClicked: {
                                            editor.insert(editor.cursorPosition, `{{ colors.${colorItem.modelData}.default.${panel.format} }}`);
                                            editor.forceActiveFocus();
                                        }
                                        contentItem: RowLayout {
                                            spacing: 8
                                            Rectangle {
                                                implicitWidth: 18
                                                implicitHeight: 18
                                                radius: 9
                                                color: panel.colors[colorItem.modelData]
                                                border.width: 1
                                                border.color: Appearance.colors.colOutlineVariant
                                            }
                                            StyledText {
                                                Layout.fillWidth: true
                                                elide: Text.ElideRight
                                                font.pixelSize: Appearance.font.pixelSize.smaller
                                                color: Appearance.colors.colOnLayer2
                                                text: colorItem.modelData
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 3. What it renders to.
                    Rectangle {
                        anchors.fill: parent
                        visible: panel.step === "preview"
                        radius: Appearance.rounding.normal
                        color: Appearance.colors.colLayer2
                        clip: true
                        Flickable {
                            id: previewFlick
                            anchors.fill: parent
                            anchors.margins: 8
                            contentWidth: previewText.contentWidth
                            contentHeight: previewText.contentHeight
                            clip: true
                            StyledTextArea {
                                id: previewText
                                width: Math.max(previewFlick.width, implicitWidth)
                                readOnly: true
                                wrapMode: TextEdit.NoWrap
                                font.family: Appearance.font.family.monospace
                                background: null
                                text: panel.preview
                                SyntaxHighlighter {
                                    textEdit: previewText
                                    repository: Repository
                                    definition: Repository.definitionForFileName(panel.output)
                                    theme: Appearance.syntaxHighlightingTheme
                                }
                            }
                        }
                    }
                }

                StyledText {
                    Layout.fillWidth: true
                    visible: panel.error !== ""
                    wrapMode: Text.Wrap
                    color: Appearance.colors.colError
                    text: panel.error
                }

                WindowDialogButtonRow {
                    Layout.bottomMargin: 10
                    DialogButton {
                        visible: panel.step !== "pick"
                        buttonText: Translation.tr("Back")
                        onClicked: {
                            panel.error = "";
                            panel.step = panel.step === "preview" ? "edit" : "pick";
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    DialogButton {
                        buttonText: Translation.tr("Cancel")
                        onClicked: panel.close()
                    }
                    DialogButton {
                        visible: panel.step === "pick"
                        buttonText: Translation.tr("Next")
                        enabled: panel.validId && panel.output !== ""
                        onClicked: panel.startEditing()
                    }
                    DialogButton {
                        visible: panel.step === "edit"
                        buttonText: Translation.tr("Preview")
                        enabled: !previewProc.running
                        onClicked: {
                            panel.error = "";
                            panel.preview = "";
                            panel.step = "preview";
                            previewProc.running = true;
                        }
                    }
                    DialogButton {
                        visible: panel.step === "preview"
                        buttonText: Translation.tr("Save")
                        enabled: panel.error === "" && !saveProc.running && !previewProc.running
                        onClicked: saveProc.running = true
                    }
                }
            }
        }
    }
}
