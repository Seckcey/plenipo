; Plenipo's installer hooks (Phase 13; ADR-037 background work, ADR-038 updates). Tauri's
; installer includes this file and runs these macros at its own points.

; Before files change, a running Plenipo is asked to quit the normal way, so its work is stopped
; and recorded, instead of the installer ending it at once. Only 1.9.0 and newer understand
; --quit: an older one would open a second Plenipo instead, so it is left to the installer's own
; check. During an update Plenipo has already stopped its work and is quitting: just wait.
!macro PLENIPO_QUIT_RUNNING
  nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
  Pop $R9
  ; $R5 = 1: Plenipo is quitting by itself (asked here, or an update), so wait for it.
  StrCpy $R5 0
  ${If} $UpdateMode = 1
    StrCpy $R5 1
  ${EndIf}
  ${If} $R9 = 0
    ${If} $UpdateMode <> 1
      ReadRegStr $R8 SHCTX "${UNINSTKEY}" "DisplayVersion"
      ${If} $R8 != ""
      ${AndIf} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
        nsis_tauri_utils::SemverCompare "$R8" "1.9.0"
        Pop $R7
        ${If} $R7 >= 0
          DetailPrint "Asking Plenipo to stop its work and quit..."
          Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --quit'
          StrCpy $R5 1
        ${EndIf}
      ${EndIf}
    ${EndIf}
  ${EndIf}
  ; Wait up to 30 seconds for it to finish. An older Plenipo that was not asked is left to the
  ; installer's own check, which offers to close it.
  ${If} $R9 = 0
  ${AndIf} $R5 = 1
    StrCpy $R6 0
    ${Do}
      Sleep 500
      IntOp $R6 $R6 + 1
      nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
      Pop $R9
      ${If} $R9 <> 0
        ${ExitDo}
      ${EndIf}
      ${If} $R6 >= 60
        ${ExitDo}
      ${EndIf}
    ${Loop}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro PLENIPO_QUIT_RUNNING
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; For silent uninstalls (and the Windows tests): /DELETEAPPDATA ticks "Also delete my Plenipo
  ; data" without the window.
  ${GetOptions} $CMDLINE "/DELETEAPPDATA" $R0
  ${IfNot} ${Errors}
    StrCpy $DeleteAppDataCheckboxState 1
  ${EndIf}
  !insertmacro PLENIPO_QUIT_RUNNING
  ; Deleting the data also forgets the secrets Plenipo kept in Windows Credential Manager (they
  ; are not in Plenipo's folder). Plenipo does it itself, before its files are removed.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
  ${AndIf} ${FileExists} "$INSTDIR\${MAINBINARYNAME}.exe"
    DetailPrint "Removing the secrets Plenipo kept in Windows Credential Manager..."
    nsExec::Exec '"$INSTDIR\${MAINBINARYNAME}.exe" --plenipo-forget-secrets'
    Pop $R9
  ${EndIf}
!macroend
