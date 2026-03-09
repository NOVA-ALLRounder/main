mod artifacts;
mod parsing;

pub(crate) use self::artifacts::detect_artifact_evidence_assertions;
pub(crate) use self::parsing::stamp_run_scope_evidence;
pub(super) use self::parsing::{
    latest_evidence_field, latest_evidence_fields, latest_evidence_int,
    latest_legacy_mail_send_field, logs_have_evidence_fields,
};
