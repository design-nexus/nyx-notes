import QtQuick
import "nyx/Ui" as Nyx
Card {
 id:root
 property var note:({})
 interactive:true
 color:hovered?Nyx.Style.rowHover:"transparent"
 Accessible.name:note.title||"Note"
 Plain {width:parent.width;text:(root.note.pinned?"⌖  ":"")+(root.note.title||"Untitled");strong:true}
 Plain {width:parent.width;text:root.note.path||"";dim:true;role:"caption"}
 Plain {width:parent.width;text:root.note.preview||"";dim:true;role:"caption";maximumLineCount:2;visible:text!==""}
}
