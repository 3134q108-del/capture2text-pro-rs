; Capture2Text Pro NSIS installer hooks

!include LogicLib.nsh
!define MODELS_DIR "$LOCALAPPDATA\com.capture2text.pro\models"

!macro NSIS_HOOK_PREINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; reserved
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "${MODELS_DIR}\*.gguf" 0 skip_model_dialog

  MessageBox MB_YESNOCANCEL|MB_ICONQUESTION \
    "是否同時刪除已下載的 AI 模型?$\r$\n$\r$\n是 = 全部刪除$\r$\n否 = 保留$\r$\n取消 = 部分選擇" \
    /SD IDNO IDYES delete_all_models IDNO keep_models

  Goto partial_select_models

delete_all_models:
  RMDir /r "${MODELS_DIR}"
  Goto skip_model_dialog

partial_select_models:
  IfFileExists "${MODELS_DIR}\qwen3-vl-2b-instruct.Q4_K_M.gguf" 0 ask_4b
    MessageBox MB_YESNO|MB_ICONQUESTION "刪除 Qwen3-VL-2B-Instruct (1.5 GB)?" /SD IDNO IDYES delete_2b
    Goto ask_4b
  delete_2b:
    Delete "${MODELS_DIR}\qwen3-vl-2b-instruct.Q4_K_M.gguf"
    Delete "${MODELS_DIR}\qwen3-vl-2b-instruct.mmproj.gguf"

ask_4b:
  IfFileExists "${MODELS_DIR}\qwen3-vl-4b-instruct.Q4_K_M.gguf" 0 ask_8b
    MessageBox MB_YESNO|MB_ICONQUESTION "刪除 Qwen3-VL-4B-Instruct (2.5 GB)?" /SD IDNO IDYES delete_4b
    Goto ask_8b
  delete_4b:
    Delete "${MODELS_DIR}\qwen3-vl-4b-instruct.Q4_K_M.gguf"
    Delete "${MODELS_DIR}\qwen3-vl-4b-instruct.mmproj.gguf"

ask_8b:
  IfFileExists "${MODELS_DIR}\qwen3-vl-8b-instruct.Q4_K_M.gguf" 0 cleanup_check
    MessageBox MB_YESNO|MB_ICONQUESTION "刪除 Qwen3-VL-8B-Instruct (5 GB)?" /SD IDNO IDYES delete_8b
    Goto cleanup_check
  delete_8b:
    Delete "${MODELS_DIR}\qwen3-vl-8b-instruct.Q4_K_M.gguf"
    Delete "${MODELS_DIR}\qwen3-vl-8b-instruct.mmproj.gguf"

cleanup_check:
  RMDir "${MODELS_DIR}"
  Goto skip_model_dialog

keep_models:
  ; keep downloaded models

skip_model_dialog:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; reserved
!macroend
