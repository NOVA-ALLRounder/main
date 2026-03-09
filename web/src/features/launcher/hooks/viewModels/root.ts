import type { PanelBuilderArgs } from "@/features/launcher/hooks/viewModels/shared";

export type LauncherRootProps = {
    onMouseDown: PanelBuilderArgs["actions"]["handleBackgroundClick"];
    launcherWidthClass: string;
    successPulse: boolean;
    shake: boolean;
};

export function buildLauncherRootProps({
    runtime,
    actions,
}: PanelBuilderArgs): LauncherRootProps {
    const launcherWidthClass = runtime.compactLayoutMode
        ? "max-w-[calc(100vw-16px)] sm:max-w-[calc(100vw-24px)] lg:max-w-[1080px] xl:max-w-[1160px]"
        : "max-w-[calc(100vw-16px)] sm:max-w-[calc(100vw-24px)] lg:max-w-[1220px] xl:max-w-[1280px]";

    return {
        onMouseDown: actions.handleBackgroundClick,
        launcherWidthClass,
        successPulse: runtime.successPulse,
        shake: runtime.shake,
    };
}
