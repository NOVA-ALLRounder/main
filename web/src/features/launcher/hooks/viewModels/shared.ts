import type { LauncherActions } from "@/features/launcher/hooks/useLauncherActions";
import type { LauncherRuntimeView } from "@/features/launcher/hooks/useLauncherRuntimeView";

export type PanelBuilderArgs = {
    runtime: LauncherRuntimeView;
    actions: LauncherActions;
};
