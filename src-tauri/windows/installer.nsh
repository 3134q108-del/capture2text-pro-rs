; Capture2Text Pro NSIS installer hooks
!define NSJSON_PLUGIN_DIR "${__FILEDIR__}\plugins\nsJSON\Plugins\x86-unicode"

!macro NSIS_HOOK_PREINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; UI-only task: no deletion here
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "[uninstall-mode] $UninstallMode"
  DetailPrint "[partial-items] $ItemChecked"
  DetailPrint "[partial-webview] $DeleteWebView"
!macroend
