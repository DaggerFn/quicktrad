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
  property bool pillFlash: false

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

  // Mesmo efeito visual da janela flutuante: o par pisca e a seta gira por
  // um instante, deixando a inversão óbvia mesmo sem nenhum elemento extra
  // na UI. spinTurns incrementa (em vez de resetar pra 0) porque Behavior
  // anima a partir do valor atual — resetar faria a seta "voltar" em vez
  // de continuar girando pro mesmo lado a cada swap.
  property int spinTurns: 0
  function flashPill() {
    spinTurns += 1
    pillFlash = true
    flashResetTimer.restart()
  }

  Timer {
    id: flashResetTimer
    interval: 160
    onTriggered: root.pillFlash = false
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
        root.flashPill()
        // A tradução que já estava pronta é o texto no novo idioma de
        // origem — sobe ela pro campo (em vez de reprocessar o texto
        // antigo, que ainda está no idioma errado pro novo par).
        if (root.resultText.trim() !== "") {
          inputField.text = root.resultText
        }
        root.resultText = ""
        root.resultError = ""
        if (inputField.text.trim() !== "") {
          debounce.stop()
          root.runQuery(inputField.text)
        }
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
      // Sem isso, o keyCatcher intercepta TODA tecla antes do campo de
      // texto (Keys.priority: Keys.BeforeItem) — inclusive Tab, que ele já
      // consome pra própria navegação entre painéis. Com o campo focado,
      // deixa a tecla passar direto pro TextField.
      blocked: inputField.activeFocus
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
            color: root.pillFlash ? root.bar.foreground : Qt.darker(root.bar.foreground, 1.4)
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.caption
            font.bold: true
            anchors.right: swapBtn.left
            anchors.rightMargin: Style.space(8)
            anchors.verticalCenter: parent.verticalCenter

            Behavior on color { ColorAnimation { duration: 220; easing.type: Easing.OutCubic } }
          }

          Text {
            id: swapBtn
            text: "⇄"
            color: root.bar.foreground
            font.family: root.bar.fontFamily
            font.pixelSize: Style.font.title
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            rotation: root.spinTurns * 180

            Behavior on rotation { NumberAnimation { duration: 380; easing.type: Easing.OutCubic } }

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
          // Tab inverte o par atual — mesma tecla da janela flutuante.
          Keys.onTabPressed: root.runSwap()
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
