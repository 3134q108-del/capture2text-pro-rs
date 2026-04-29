; Capture2Text Pro NSIS installer hooks
; Tauri 2 注入點: NSIS_HOOK_PREUNINSTALL (主要邏輯), 其他 hook 暫留空殼

!macro NSIS_HOOK_PREINSTALL
  ; 暫不需要
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; 暫不需要
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 升級流程: Tauri bundler 在 upgrade 時呼叫舊 uninstaller 加 /UPDATE flag,
  ; NSIS 模板會 set ${UpdateMode} (從 Tauri tauri-bundler 看到的 mechanism)
  ; 升級不該問 user, 直接保留資料
  ${If} $UpdateMode = 1
    Goto skip_data_dialog
  ${EndIf}

  ; Silent uninstall (/S): 走預設保留資料分支
  IfSilent skip_data_dialog 0

  ; 主動 uninstall: 顯示 dialog
  ; MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 = Yes/No 互動, 預設按鈕為 No
  ; /SD IDNO = silent default 也是 No (雙保險, 雖然 IfSilent 已先擋)
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 \
    "是否同時刪除使用者資料?$\r$\n$\r$\n包含: 設定、自訂熱鍵、Azure 設定、OCR 截圖記錄、TTS 快取、模型 (~6 GB)$\r$\n位置: $LOCALAPPDATA\Capture2TextPro\$\r$\n$\r$\n選擇否會保留資料, 下次重裝可直接用 (不需重下模型)。" \
    /SD IDNO \
    IDYES delete_data IDNO skip_data_dialog
  Goto skip_data_dialog

  delete_data:
    DetailPrint "正在刪除使用者資料 ..."
    RMDir /r "$LOCALAPPDATA\Capture2TextPro"
    DetailPrint "使用者資料已清除"

  skip_data_dialog:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; 暫不需要
!macroend
