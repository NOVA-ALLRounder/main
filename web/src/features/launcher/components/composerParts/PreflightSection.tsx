import {
  ComposerExecutionLockHint,
  getPreflightPanelClass,
  PreflightChecksGrid,
  PreflightHeader,
  type ComposerPreflightSectionProps,
} from "@/features/launcher/components/composerParts/preflightParts";

export function ComposerPreflightSection(props: ComposerPreflightSectionProps) {
  if (props.ui.composerMode === "chat") {
    return null;
  }

  return (
    <>
      <ComposerExecutionLockHint ui={props.ui} />

      {props.preflight.showPanel && (
        <div
          className={`mt-2.5 rounded-xl border px-3 py-2 ${getPreflightPanelClass(props.preflight)}`}
        >
          <PreflightHeader {...props} />
          <PreflightChecksGrid
            ui={props.ui}
            preflight={props.preflight}
            handlers={props.handlers}
          />
        </div>
      )}
    </>
  );
}
