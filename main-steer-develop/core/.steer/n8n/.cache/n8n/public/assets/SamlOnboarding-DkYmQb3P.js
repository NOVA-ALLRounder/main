import { It as ref, P as defineComponent, Pt as reactive, T as createBlock, et as openBlock } from "./vue.runtime.esm-bundler-XtMkEjzB.js";
import { _t as useI18n } from "./_MapCache-D-6DBsTy.js";
import "./src-BjCe3xxd.js";
import { y as useRouter } from "./truncate-CiSH3Avz.js";
import { br as useToast, t as useUsersStore } from "./users.store-p-3LYLwz.js";
import "./sanitize-html-DeDnsMgc.js";
import "./empty-nq5-pHAR.js";
import { zo as VIEWS } from "./constants-C6S5WPU1.js";
import "./merge-DzkNhZhD.js";
import "./_baseOrderBy-BY4x0dN2.js";
import "./dateformat-BeHi9sF4.js";
import "./useDebounce-CHQ-VByC.js";
import "./sso.store-DcIsixuE.js";
import { t as AuthView_default } from "./AuthView-B0yuPPZ-.js";
var SamlOnboarding_default = /* @__PURE__ */ defineComponent({
	__name: "SamlOnboarding",
	setup(__props) {
		const router = useRouter();
		const locale = useI18n();
		const toast = useToast();
		const usersStore = useUsersStore();
		const loading = ref(false);
		const FORM_CONFIG = reactive({
			title: locale.baseText("auth.signup.setupYourAccount"),
			buttonText: locale.baseText("auth.signup.finishAccountSetup"),
			inputs: [{
				name: "firstName",
				initialValue: usersStore.currentUser?.firstName,
				properties: {
					label: locale.baseText("auth.firstName"),
					maxlength: 32,
					required: true,
					autocomplete: "given-name",
					capitalize: true
				}
			}, {
				name: "lastName",
				initialValue: usersStore.currentUser?.lastName,
				properties: {
					label: locale.baseText("auth.lastName"),
					maxlength: 32,
					required: true,
					autocomplete: "family-name",
					capitalize: true
				}
			}]
		});
		const isFormWithFirstAndLastName = (values) => {
			return "firstName" in values && "lastName" in values;
		};
		const onSubmit = async (values) => {
			if (!isFormWithFirstAndLastName(values)) return;
			try {
				loading.value = true;
				await usersStore.updateUserName(values);
				await router.push({ name: VIEWS.HOMEPAGE });
			} catch (error) {
				loading.value = false;
				toast.showError(error, "Error", error.message);
			}
		};
		return (_ctx, _cache) => {
			return openBlock(), createBlock(AuthView_default, {
				form: FORM_CONFIG,
				"form-loading": loading.value,
				onSubmit
			}, null, 8, ["form", "form-loading"]);
		};
	}
});
export { SamlOnboarding_default as default };
