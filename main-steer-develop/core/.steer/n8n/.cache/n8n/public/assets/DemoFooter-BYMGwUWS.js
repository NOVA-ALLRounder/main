import { C as computed, E as createCommentVNode, P as defineComponent, T as createBlock, et as openBlock } from "./vue.runtime.esm-bundler-XtMkEjzB.js";
import "./_MapCache-D-6DBsTy.js";
import "./src-BjCe3xxd.js";
import "./truncate-CiSH3Avz.js";
import { s as useWorkflowsStore } from "./users.store-p-3LYLwz.js";
import "./sanitize-html-DeDnsMgc.js";
import "./empty-nq5-pHAR.js";
import "./constants-C6S5WPU1.js";
import "./merge-DzkNhZhD.js";
import "./_baseOrderBy-BY4x0dN2.js";
import "./dateformat-BeHi9sF4.js";
import "./useDebounce-CHQ-VByC.js";
import "./useClipboard-CnA0WmDD.js";
import "./executions.store-Di1iqSdi.js";
import "./assistant.store-Dnyzj91N.js";
import "./chatPanel.store-DH2wOLdY.js";
import "./RunData-DqmR6M3x.js";
import "./NDVEmptyState-SCsOPB6e.js";
import "./externalSecrets.ee.store-ClgEyoPT.js";
import "./usePinnedData-BATZCDOK.js";
import "./nodeCreator.store-CATijmG4.js";
import "./canvas.utils-DzdCZb0z.js";
import "./nodeIcon-BhljGdvK.js";
import "./useCanvasOperations-qhJysCtj.js";
import { t as LogsPanel_default } from "./LogsPanel-9deSOewk.js";
import "./folders.store-DVkp6gLO.js";
import "./pushConnection.store-yREZVaYW.js";
import "./RunDataHtml-BMxc-zRm.js";
import "./NodeIcon-Bja_Y7pf.js";
import "./useRunWorkflow-a0FS5Kub.js";
import "./vue-json-pretty-CTqCbq0T.js";
import "./collaboration.store-g-w4G2Tj.js";
import "./dateFormatter-DaEPHRxX.js";
import "./useExecutionHelpers-BqGccVki.js";
import "./KeyboardShortcutTooltip-T0sCfWlL.js";
import "./useKeybindings-BLi9_NZz.js";
import "./ChatFile-DGN9Z6P_.js";
import "./AnimatedSpinner-B25lpnvb.js";
import "./useLogsTreeExpand-mYz24HBF.js";
var DemoFooter_default = /* @__PURE__ */ defineComponent({
	__name: "DemoFooter",
	setup(__props) {
		const workflowsStore = useWorkflowsStore();
		const hasExecutionData = computed(() => workflowsStore.workflowExecutionData);
		return (_ctx, _cache) => {
			return hasExecutionData.value ? (openBlock(), createBlock(LogsPanel_default, {
				key: 0,
				"is-read-only": true
			})) : createCommentVNode("", true);
		};
	}
});
export { DemoFooter_default as default };
