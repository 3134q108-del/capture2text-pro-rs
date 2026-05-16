; Capture2Text Pro NSIS installer hooks
; installer.nsi is generated under src-tauri/target/*/nsis/x64 before makensis runs.
!define NSJSON_PLUGIN_DIR "..\..\..\..\windows\plugins\nsJSON\Plugins\x86-unicode"
!define ROAMING_DIR "$APPDATA\${BUNDLEID}"
!define LOCAL_DIR "$LOCALAPPDATA\${BUNDLEID}"
!define WEBVIEW_DIR "${LOCAL_DIR}\EBWebView"
!define CACHE_DIR "${LOCAL_DIR}\Cache"

!macro NSIS_HOOK_PREINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UninstallMode == "full"
    DetailPrint "[cleanup] attempting credential cleanup..."
    nsExec::ExecToStack 'cmdkey /delete:Capture2TextPro'
    Pop $0
    Pop $1
    DetailPrint "[cleanup] cmdkey delete:Capture2TextPro exit=$0"

    nsExec::ExecToStack 'cmdkey /delete:LegacyGeneric:target=Capture2TextPro'
    Pop $0
    Pop $1
    DetailPrint "[cleanup] cmdkey delete:LegacyGeneric exit=$0"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "[uninstall-mode] $UninstallMode"
  DetailPrint "[partial-items] $ItemChecked"
  DetailPrint "[partial-webview] $DeleteWebView"

  ${If} $UninstallMode == "minimal"
    IfFileExists "${WEBVIEW_DIR}\*.*" 0 +4
      Sleep 500
      RMDir /r "${WEBVIEW_DIR}"
      DetailPrint "[cleanup] removed ${WEBVIEW_DIR}"
    IfFileExists "${CACHE_DIR}\*.*" 0 +3
      RMDir /r "${CACHE_DIR}"
      DetailPrint "[cleanup] removed ${CACHE_DIR}"

  ${ElseIf} $UninstallMode == "partial"
    ${If} $ItemChecked != ""
      ${WordFind} "$ItemChecked" "|" "#" $0
      ${For} $1 1 $0
        ${WordFind} "$ItemChecked" "|" "+$1" $2
        ${WordFind} "$2" ":" "+1" $3
        ${WordFind} "$2" ":" "+2" $4
        ${If} $4 != ""
          StrCpy $5 "$4" 1 -1
          ${If} $5 == "/"
          ${OrIf} $5 == "\"
            IfFileExists "${LOCAL_DIR}\$4\*.*" 0 +3
              RMDir /r "${LOCAL_DIR}\$4"
              DetailPrint "[cleanup] partial removed dir id=$3 path=$4"
          ${Else}
            IfFileExists "${LOCAL_DIR}\$4" 0 +3
              Delete "${LOCAL_DIR}\$4"
              DetailPrint "[cleanup] partial removed file id=$3 path=$4"
          ${EndIf}
        ${EndIf}
      ${Next}
    ${EndIf}

    ${If} $DeleteWebView == "1"
      IfFileExists "${WEBVIEW_DIR}\*.*" 0 +4
        Sleep 500
        RMDir /r "${WEBVIEW_DIR}"
        DetailPrint "[cleanup] partial removed ${WEBVIEW_DIR}"
    ${EndIf}

    RMDir "${LOCAL_DIR}\models"
    RMDir "${LOCAL_DIR}\captures"
    RMDir "${LOCAL_DIR}\tts_preview_cache"
    RMDir "${LOCAL_DIR}\tts_speak_cache"

  ${Else}
    IfFileExists "${ROAMING_DIR}\*.*" 0 +3
      RMDir /r "${ROAMING_DIR}"
      DetailPrint "[cleanup] removed ${ROAMING_DIR}"
    IfFileExists "${LOCAL_DIR}\*.*" 0 +3
      RMDir /r "${LOCAL_DIR}"
      DetailPrint "[cleanup] removed ${LOCAL_DIR}"
  ${EndIf}
!macroend
