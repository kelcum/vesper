; Vesper installer hooks (included by Tauri's NSIS template).
;
; Uninstall leaves Roblox exactly as Vesper found it:
;   * `Vesper.exe --cleanup` removes the ClientAppSettings.json files Vesper
;     wrote (only files whose content still matches what Vesper wrote) and
;     restores Roblox's original frame-rate cap.
;   * If "Delete the application data" is ticked, Vesper's own folders go too.
; Neither runs during an in-place update.

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    IfFileExists "$INSTDIR\Vesper.exe" 0 +2
      ExecWait '"$INSTDIR\Vesper.exe" --cleanup'
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    ; Tauri removes $LOCALAPPDATA\<identifier> (logs, cache, WebView2 data);
    ; settings, profiles and backups live here.
    SetShellVarContext current
    RmDir /r "$APPDATA\Vesper"
  ${EndIf}
!macroend
