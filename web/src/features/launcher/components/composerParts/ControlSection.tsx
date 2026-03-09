import {
  ComposerAdvancedToggle,
  ComposerModeSwitch,
  ComposerProfileControls,
  ComposerProfileOptions,
  type ComposerControlRowProps,
} from "@/features/launcher/components/composerParts/controlParts";

export function ComposerControlRow(props: ComposerControlRowProps) {
  return (
    <>
      <ComposerModeSwitch ui={props.ui} handlers={props.handlers} />
      <ComposerAdvancedToggle ui={props.ui} handlers={props.handlers} />
      <ComposerProfileOptions {...props} />
      <ComposerProfileControls {...props} />
    </>
  );
}
