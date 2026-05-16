; Capture2Text Pro NSIS installer hooks
!define ROAMING_DIR "$APPDATA\com.capture2text.pro"
!define LOCAL_DIR "$LOCALAPPDATA\com.capture2text.pro"
!define WEBVIEW_DIR "${LOCAL_DIR}\EBWebView"
!define CACHE_DIR "${LOCAL_DIR}\Cache"

!macro NSIS_HOOK_PREINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  SetShellVarContext current
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
  SetShellVarContext current
  DetailPrint "[uninstall-mode] $UninstallMode"
  DetailPrint "[partial-flags] models=$DeleteModels captures=$DeleteCaptures settings=$DeleteSettings tts_cache=$DeleteTtsCache logs=$DeleteLogs webview=$DeleteWebView"

  ${If} $UninstallMode == "minimal"
    IfFileExists "${WEBVIEW_DIR}\*.*" 0 +4
      Sleep 500
      RMDir /r "${WEBVIEW_DIR}"
      DetailPrint "[cleanup] removed ${WEBVIEW_DIR}"
    IfFileExists "${CACHE_DIR}\*.*" 0 +3
      RMDir /r "${CACHE_DIR}"
      DetailPrint "[cleanup] removed ${CACHE_DIR}"

  ${ElseIf} $UninstallMode == "partial"
    ${If} $DeleteModels == "1"
      IfFileExists "${LOCAL_DIR}\models\*.*" 0 +2
        RMDir /r "${LOCAL_DIR}\models"
        DetailPrint "[cleanup] partial removed ${LOCAL_DIR}\models"
    ${EndIf}
    ${If} $DeleteCaptures == "1"
      IfFileExists "${LOCAL_DIR}\captures\*.*" 0 +2
        RMDir /r "${LOCAL_DIR}\captures"
        DetailPrint "[cleanup] partial removed ${LOCAL_DIR}\captures"
      IfFileExists "${LOCAL_DIR}\captures.log" 0 +2
        Delete "${LOCAL_DIR}\captures.log"
        DetailPrint "[cleanup] partial removed ${LOCAL_DIR}\captures.log"
    ${EndIf}
    ${If} $DeleteSettings == "1"
      Delete "${LOCAL_DIR}\scenarios.json"
      Delete "${LOCAL_DIR}\window_state.json"
      Delete "${LOCAL_DIR}\output_lang.txt"
      Delete "${LOCAL_DIR}\tts_config.json"
      DetailPrint "[cleanup] partial removed settings files"
    ${EndIf}
    ${If} $DeleteTtsCache == "1"
      IfFileExists "${LOCAL_DIR}\tts_preview_cache\*.*" 0 +2
        RMDir /r "${LOCAL_DIR}\tts_preview_cache"
        DetailPrint "[cleanup] partial removed ${LOCAL_DIR}\tts_preview_cache"
      IfFileExists "${LOCAL_DIR}\tts_speak_cache\*.*" 0 +2
        RMDir /r "${LOCAL_DIR}\tts_speak_cache"
        DetailPrint "[cleanup] partial removed ${LOCAL_DIR}\tts_speak_cache"
    ${EndIf}
    ${If} $DeleteLogs == "1"
      IfFileExists "${LOCAL_DIR}\tts_debug\*.*" 0 +2
        RMDir /r "${LOCAL_DIR}\tts_debug"
        DetailPrint "[cleanup] partial removed ${LOCAL_DIR}\tts_debug"
      IfFileExists "${LOCAL_DIR}\leptonica_check\*.*" 0 +2
        RMDir /r "${LOCAL_DIR}\leptonica_check"
        DetailPrint "[cleanup] partial removed ${LOCAL_DIR}\leptonica_check"
    ${EndIf}
    ${If} $DeleteWebView == "1"
      IfFileExists "${WEBVIEW_DIR}\*.*" 0 +4
        Sleep 500
        RMDir /r "${WEBVIEW_DIR}"
        DetailPrint "[cleanup] partial removed ${WEBVIEW_DIR}"
    ${EndIf}

  ${Else}
    IfFileExists "${ROAMING_DIR}\*.*" 0 roaming_miss
      RMDir /r "${ROAMING_DIR}"
      DetailPrint "[cleanup] removed ${ROAMING_DIR}"
      Goto roaming_done
    roaming_miss:
    roaming_done:

    IfFileExists "${LOCAL_DIR}\*.*" 0 local_miss
      ClearErrors
      RMDir /r "${LOCAL_DIR}"
      DetailPrint "[cleanup] removed ${LOCAL_DIR}"
      Goto local_done
    local_miss:
    local_done:
  ${EndIf}
!macroend
