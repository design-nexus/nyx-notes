import QtQuick
import qs.Commons
import qs.Ui
Panel {
 id:root
 moduleName:"design-nexus.notes"
 ipcTarget:"design-nexus.notes"
 implicitWidth:button.implicitWidth
 implicitHeight:button.implicitHeight
 BarIconButton {id:button;anchors.fill:parent;bar:root.bar;text:"▤";tooltipText:"Notes";active:root.opened;onPressed:root.toggle()}
 KeyboardPanel {
  id:popup
  padding:0
  bar:root.bar;owner:root;anchorItem:button;open:root.opened
  contentWidth:popup.fittedContentWidth(400)
  contentHeight:popup.fittedContentHeight(600,600)
  focusTarget:content
  NotesPanel {id:content;anchors.fill:parent;active:root.opened;focus:true;onCloseRequested:root.close()}
 }
}
