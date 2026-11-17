import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
Scope {
 id:root
 property bool opened:false
 IpcHandler {target:"nyx-notes-panel";function toggle():void{root.opened=!root.opened}
 function show():void{root.opened=true}
 function hide():void{content.closeRequested()}}
 PanelWindow {
  visible:root.opened
  anchors.top:true;anchors.right:true
  margins.top:48;margins.right:18
  implicitWidth:Math.min(400,screen?screen.width-24:400)
  implicitHeight:Math.min(600,screen?screen.height-80:600)
  color:"transparent"
  exclusiveZone:0
  WlrLayershell.layer:WlrLayer.Overlay
  WlrLayershell.keyboardFocus:WlrKeyboardFocus.OnDemand
  NotesPanel {id:content;anchors.fill:parent;active:root.opened;focus:true;onCloseRequested:root.opened=false}
 }
}
