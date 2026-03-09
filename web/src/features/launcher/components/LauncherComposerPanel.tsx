import {
    type LauncherComposerPanelProps,
    ComposerControlRow,
    ComposerInputRow,
    ComposerPreflightSection,
    ComposerQuickStripSection,
    ComposerToolbarSection,
} from "@/features/launcher/components/LauncherComposerSections";

export type { LauncherComposerPanelProps } from "@/features/launcher/components/LauncherComposerSections";

export function LauncherComposerPanel(props: LauncherComposerPanelProps) {
    return (
        <div className="launcher-composer px-4 py-3.5 bg-[#141820]">
            <ComposerControlRow ui={props.ui} profile={props.profile} handlers={props.handlers} />
            <div className="flex items-center gap-3">
                <ComposerInputRow refs={props.refs} ui={props.ui} handlers={props.handlers} />
            </div>
            <ComposerPreflightSection
                ui={props.ui}
                profile={props.profile}
                preflight={props.preflight}
                handlers={props.handlers}
            />
            <ComposerQuickStripSection
                ui={props.ui}
                quickActions={props.quickActions}
                handlers={props.handlers}
            />
            <ComposerToolbarSection
                ui={props.ui}
                runtime={props.runtime}
                handlers={props.handlers}
            />
        </div>
    );
}
