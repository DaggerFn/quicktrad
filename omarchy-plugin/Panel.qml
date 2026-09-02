import QtQuick
import Quickshell.Io
import qs.Commons
import qs.Ui

// Widget de barra do quicktrad: ícone + card ancorado (igual wifi/bateria)
// com um campo de texto e o resultado da tradução. Nada de lógica de
// tradução aqui — todo o trabalho pesado (providers, config, idiomas) mora
// no binário Rust, chamado como subprocesso em modo headless
// (`quicktrad --query/--swap/--status`). Isso mantém uma única fonte de
// verdade compartilhada com a janela flutuante (mesmo config.toml), sem
// duplicar a lógica de tradução em QML.
Panel {
  id: root
  moduleName: "guts.quicktrad"
  ipcTarget: "guts.quicktrad"

  property string sourceLang: ""
  property string targetLang: ""
  property string resultText: ""
  property string resultError: ""
  property bool querying: false
  property int querySeq: 0

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  function applyStatusLine(line) {
    var parts = String(line || "").trim().split(/\s+/)
    if (parts.length < 2) return
    root.sourceLang = parts[0]
    root.targetLang = parts[1]
  }

  function refreshStatus() {
    if (statusProc.running) return
    statusProc.running = true
  }

  function runQuery(text) {
    root.querySeq++
    var seq = root.querySeq
    if (text.trim() === "") {
      root.resultText = ""
      root.resultError = ""
      root.querying = false
      return
    }
    root.querying = true
    if (queryProc.running) queryProc.running = false
    queryProc.expectedSeq = seq
    queryProc.command = ["quicktrad", "--query", text]
    queryProc.running = true
  }

  function runSwap() {
    if (swapProc.running) return
    swapProc.running = true
  }

  onOpenedChanged: {
    if (opened) {
      inputField.text = ""
      resultText = ""
      resultError = ""
      refreshStatus()
    } else if (queryProc.running) {
      queryProc.running = false
    }
  }

  Process {
    id: statusProc
    command: ["quicktrad", "--status"]
    stdout: StdioCollector { waitForEnd: true; onStreamFinished: root.applyStatusLine(text) }
  }

  Process {
    id: swapProc
    command: ["quicktrad", "--swap"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.applyStatusLine(text)
        if (inputField.text.trim() !== "") root.runQuery(inputField.text)
      }
    }
  }

  Process {
    id: queryProc
    property int expectedSeq: 0
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        if (queryProc.expectedSeq !== root.querySeq) return
        root.resultText = String(text || "").trim()
        root.resultError = ""
        root.querying = false
      }
    }
    stderr: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        if (queryProc.expectedSeq !== root.querySeq) return
        var msg = String(text || "").trim()
        if (msg !== "") {
          root.resultError = msg
          root.resultText = ""
          root.querying = false
        }
      }
    }
  }

  Timer {
    id: debounce
    interval: 350
    onTriggered: root.runQuery(inputField.text)
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: "TR"
    tooltipText: "Quicktrad"
    onPressed: function(b) { root.toggle() }
  }

  KeyboardPanel {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: inputField
    contentWidth: panel.fittedContentWidth(Style.space(320))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onCloseRequested: root.close()

      Column {
        id: column
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        spacing: Style.space(10)

        Item {
          width: parent.width
          implicitHeight: title.implicitHeight

          Text {
            id: title
            text: "Quicktrad"
            color: root.bar.foreground
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.title
            font.bold: true
          }

          Text {
            id: pairLabel
            text: (root.sourceLang || "…").toUpperCase() + " → " + (root.targetLang || "…").toUpperCase()
            color: Qt.darker(root.bar.foreground, 1.4)
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
            anchors.right: swapBtn.left
            anchors.rightMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter
          }

          Text {
            id: swapBtn
            text: "⇄"
            color: root.bar.foreground
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.title
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter

            MouseArea {
              anchors.fill: parent
              anchors.margins: -Style.space(4)
              cursorShape: Qt.PointingHandCursor
              onClicked: root.runSwap()
            }
          }
        }

        TextField {
          id: inputField
          width: parent.width
          placeholderText: "Digite para traduzir"
          foreground: root.bar.foreground
          onTextChanged: debounce.restart()
          Keys.onEscapePressed: root.close()
        }

        Text {
          width: parent.width
          wrapMode: Text.WordWrap
          text: root.resultError !== "" ? root.resultError
                : (root.resultText !== "" ? root.resultText
                : (root.querying ? "traduzindo…" : "Tradução"))
          color: root.resultError !== "" ? Color.urgent
                : (root.resultText !== "" ? root.bar.foreground : Qt.darker(root.bar.foreground, 1.6))
          font.family: root.bar.fontFamily
          font.pixelSize: Style.font.body
        }
      }
    }
  }
}
