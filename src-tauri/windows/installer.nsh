; Capture2Text Pro NSIS installer hooks

!macro NSIS_HOOK_PREINSTALL
  ; 暫不需要
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; 暫不需要
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; v0.2.0 起 user data 路徑對齊 BUNDLEID(com.capture2text.pro),
  ; Tauri 內建「刪除應用程式數據」checkbox 會正確刪到 model。
  ; 不再需要自訂 dialog(避免重複問 + perMachine context $LOCALAPPDATA 解析錯)。
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; 暫不需要
!macroend
