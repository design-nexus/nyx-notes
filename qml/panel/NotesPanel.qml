import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import "nyx/Ui" as Nyx

Rectangle {
    id: root
    implicitWidth: 400
    implicitHeight: 600
    color: Nyx.Style.bg
    radius: Nyx.Style.radiusDialog
    property bool active: visible
    signal closeRequested
    property string vaultName: ""
    property string vaultId: ""
    property var notes: []
    property var drafts: []
    property bool connected: false
    property bool editing: false
    property string notePath: ""
    property string savedText: ""
    property bool loading: false
    property bool saving: false
    property bool readOnly: false
    property string status: ""
    property string error: ""
    property string afterSave: ""
    property string pendingPath: ""
    property bool recovering: false
    readonly property bool dirty: editing && (body.text !== savedText || (notePath === "" && title.text.trim() !== ""))
    function list() {
        if (backend.ready)
            backend.send("list", {
                query: search.text
            });
    }
    function newNote() {
        if (dirty) {
            afterSave = "new";
            flush();
            return;
        }
        editing = true;
        notePath = "";
        title.text = "";
        body.text = "";
        savedText = "";
        status = "Draft";
        error = "";
        readOnly = false;
        title.forceActiveFocus();
    }
    function read(path) {
        if (dirty) {
            afterSave = "read";
            pendingPath = path;
            flush();
            return;
        }
        loading = true;
        backend.send("read", {
            path: path
        });
    }
    function flush() {
        debounce.stop();
        if (!dirty || saving || !connected)
            return;
        if (notePath === "" && body.text.trim() === "" && title.text.trim() === "")
            return;
        saving = true;
        status = "Saving…";
        backend.send(notePath === "" ? "create" : "save", {
            path: notePath,
            title: title.text.trim() || ("Quick note " + new Date().toISOString().replace(/[T:]/g, "-").slice(0, 19)),
            text: body.text
        });
    }
    function finishIntent() {
        if (dirty) {
            flush();
            return;
        }
        let intent = afterSave;
        afterSave = "";
        if (intent === "new")
            newNote();
        else if (intent === "read")
            read(pendingPath);
        else if (intent === "back") {
            editing = false;
            list();
        } else if (intent === "close")
            closeRequested();
        else if (intent === "open")
            backend.send("open", {
                path: notePath
            });
    }
    function back() {
        if (dirty) {
            afterSave = "back";
            flush();
        } else {
            editing = false;
            error = "";
            list();
        }
    }
    function dismiss() {
        if (dirty) {
            afterSave = "close";
            flush();
            if (error !== "" && notePath !== "")
                backend.send("draft", {
                    path: notePath,
                    text: body.text
                });
        } else
            closeRequested();
    }
    onActiveChanged: {
        if (active) {
            backend.restart();
            if (!editing)
                list();
        } else {
            loading = false;
            if (dirty)
                flush();
        }
    }
    Bridge {
        id: backend
        binary: "nyx-notes-panel"
        enabled: root.active || root.dirty || root.saving
        onReply: (op, result, failure, request) => {
            if (op === 'ready') {
                if (root.active)
                    root.list();
                return;
            }
            if (failure) {
                root.error = failure;
                root.status = root.dirty ? "Not saved · editor retained" : "Unavailable";
                root.loading = false;
                root.saving = false;
                return;
            }
            if (op === 'list') {
                root.connected = true;
                if (root.vaultId !== "" && root.vaultId !== result.vault_id && !root.dirty) {
                    root.editing = false;
                    root.notePath = "";
                }
                root.vaultId = result.vault_id || "";
                root.vaultName = result.vault || "Personal vault";
                root.notes = result.notes || [];
                root.drafts = result.drafts || [];
                if (root.editing && root.notePath !== "" && !root.dirty)
                    root.read(root.notePath);
            }
            if (op === 'read') {
                root.loading = false;
                root.editing = true;
                root.notePath = result.path;
                title.text = result.title;
                root.readOnly = !!result.read_only;
                body.text = result.text;
                root.savedText = result.recovered ? "\u0000" : result.text;
                root.recovering = !!result.recovered;
                root.error = "";
                root.status = result.recovered ? "Recovered draft · review before saving" : "Saved";
                if (result.recovered)
                    root.error = "A recovery draft was restored. Review it, then Retry save or Save a copy.";
                body.forceActiveFocus();
            }
            if (op === 'create' || op === 'save') {
                root.saving = false;
                root.savedText = request.text;
                root.notePath = result.path || root.notePath;
                if (op === 'create')
                    title.text = result.title;
                root.status = root.dirty ? "Saving…" : "Saved";
                root.error = "";
                root.recovering = false;
                if (root.dirty)
                    debounce.restart();
                else
                    root.finishIntent();
            }
            if (op === 'draft' && root.afterSave === 'close') {
                root.afterSave = "";
                root.closeRequested();
            }
        }
    }
    Timer {
        id: debounce
        interval: 650
        onTriggered: if (!root.recovering)
            root.flush()
    }
    Timer {
        id: searchDelay
        interval: 180
        onTriggered: root.list()
    }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 10
        RowLayout {
            Layout.fillWidth: true
            Layout.minimumWidth: root.width - 28
            Plain {
                text: "▤"
                color: Nyx.Style.accent
                role: "title"
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1
                Plain {
                    text: "Notes"
                    strong: true
                }
                Plain {
                    text: root.vaultName || "Your Notes vault"
                    dim: true
                    role: "caption"
                }
            }
            Nyx.IconButton {
                objectName: "new-note-button"
                icon: "document-new-symbolic"
                primary: true
                tooltip: "New note (Ctrl+N)"
                enabled: root.connected && !root.loading
                onClicked: root.newNote()
            }
            Nyx.IconButton {
                objectName: "open-notes-button"
                icon: "go-jump-symbolic"
                tooltip: "Open in full Notes"
                onClicked: {
                    if (root.dirty) {
                        root.afterSave = "open";
                        root.flush();
                    } else
                        backend.send("open", {
                            path: root.notePath
                        });
                }
            }
            Nyx.IconButton {
                objectName: "close-notes-button"
                icon: "window-close-symbolic"
                tooltip: "Close Notes panel"
                onClicked: root.dismiss()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            visible: root.error !== ""
            Notice {
                Layout.fillWidth: true
                error: true
                text: root.error
            }
            Action {
                text: "×"
                Accessible.name: "Dismiss error"
                onClicked: root.error = ""
            }
        }
        RowLayout {
            visible: root.editing && root.error !== ""
            Layout.fillWidth: true
            Action {
                text: "Retry save"
                enabled: root.dirty && !root.saving
                onClicked: {
                    root.recovering = false;
                    root.flush();
                }
            }
            Action {
                text: "Save a copy"
                enabled: root.dirty && !root.saving
                onClicked: {
                    root.saving = true;
                    backend.send("create", {
                        title: (title.text || "Recovered note") + " copy",
                        text: body.text
                    });
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: !root.editing
            spacing: 10
            Nyx.SearchField {
                id: search
                Layout.fillWidth: true
                placeholder: "Search notes"
                hint: "Ctrl F"
                onTextChanged: searchDelay.restart()
                onAccepted: if (root.notes.length)
                    root.read(root.notes[0].path)
            }
            Controls.ScrollView {
                id: listScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentWidth: availableWidth
                Column {
                    width: listScroll.availableWidth
                    spacing: 6
                    Plain {
                        text: "Recovery drafts"
                        visible: root.drafts.length > 0
                        dim: true
                        role: "caption"
                    }
                    Repeater {
                        model: root.drafts
                        Action {
                            required property string modelData
                            text: "Recover · " + modelData
                            width: parent.width
                            onClicked: root.read(modelData)
                        }
                    }
                    Plain {
                        text: "Pinned"
                        visible: root.notes.some(n => n.pinned)
                        dim: true
                        role: "caption"
                    }
                    Repeater {
                        model: root.notes.filter(n => n.pinned)
                        NoteRow {
                            required property var modelData
                            width: parent.width
                            note: modelData
                            onClicked: root.read(modelData.path)
                        }
                    }
                    Plain {
                        text: search.text ? "Search results" : "Recent"
                        dim: true
                        role: "caption"
                    }
                    Repeater {
                        model: root.notes.filter(n => !n.pinned)
                        NoteRow {
                            required property var modelData
                            width: parent.width
                            note: modelData
                            onClicked: root.read(modelData.path)
                        }
                    }
                    Plain {
                        width: parent.width
                        visible: root.notes.length === 0
                        text: root.connected ? (search.text ? "No matching notes" : "No notes yet. Create your first note.") : "Notes is unavailable. Open Notes and choose a vault to get started."
                        dim: true
                    }
                    Action {
                        text: root.connected ? "Refresh" : "Retry connection"
                        onClicked: {
                            backend.restart();
                            root.list();
                        }
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Plain {
                    Layout.fillWidth: true
                    text: "Ctrl N · capture"
                    dim: true
                    role: "caption"
                }
                Nyx.Button {
                    text: "New note"
                    primary: true
                    enabled: root.connected
                    onClicked: root.newNote()
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.editing
            spacing: 9
            Action {
                text: "‹ Notes"
                enabled: !root.saving && !root.loading
                onClicked: root.back()
            }
            Controls.TextField {
                id: title
                Layout.fillWidth: true
                placeholderText: "Note title"
                readOnly: root.notePath !== "" || root.saving
                font.family: Nyx.Style.font
                font.pixelSize: 18
                color: Nyx.Style.text
                background: Rectangle {
                    color: "transparent"
                }
                Accessible.name: "Note title"
                onTextEdited: if (root.notePath === "")
                    debounce.restart()
            }
            RowLayout {
                Layout.fillWidth: true
                Plain {
                    Layout.fillWidth: true
                    text: "Markdown · source"
                    dim: true
                    role: "caption"
                }
                Plain {
                    text: root.readOnly ? "Read only" : "Autosave"
                    dim: true
                    role: "caption"
                }
            }
            Controls.ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                Controls.TextArea {
                    id: body
                    wrapMode: TextEdit.Wrap
                    color: Nyx.Style.text
                    font.family: Nyx.Style.font
                    font.pixelSize: 13
                    textFormat: TextEdit.PlainText
                    selectByMouse: true
                    readOnly: root.readOnly || root.loading
                    placeholderText: "Write a thought…"
                    background: Rectangle {
                        color: Nyx.Style.card
                        radius: Nyx.Style.radiusCard
                    }
                    Accessible.name: "Note body"
                    onTextChanged: {
                        if (root.editing && root.dirty && !root.readOnly && !root.loading && !root.recovering) {
                            root.status = "Unsaved changes";
                            debounce.restart();
                        }
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Plain {
                    Layout.fillWidth: true
                    text: root.status
                    role: "caption"
                    color: root.error ? Nyx.Style.warning : Nyx.Style.accent
                }
                Action {
                    text: "Open in Notes ↗"
                    onClicked: {
                        if (root.dirty) {
                            root.afterSave = "open";
                            root.flush();
                        } else
                            backend.send("open", {
                                path: root.notePath
                            });
                    }
                }
            }
        }
    }
    Shortcut {
        sequence: "Ctrl+N"
        enabled: root.active && root.connected
        onActivated: root.newNote()
    }
    Shortcut {
        sequence: "Ctrl+F"
        enabled: root.active
        onActivated: {
            root.back();
            if (!root.dirty)
                search.focusInput();
        }
    }
    Keys.onEscapePressed: if (root.editing)
        root.back()
    else
        root.dismiss()
}
