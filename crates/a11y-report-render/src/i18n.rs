//! Every user-facing label, centralized so the report can be translated.

use a11y_report_core::report::CriterionStatus;
use a11y_report_core::{Confidence, Lang, Severity, Status};

pub struct Labels {
    pub colon: &'static str,
    pub report_title: &'static str,
    pub skip_link: &'static str,
    pub app: &'static str,
    pub build: &'static str,
    pub captured_at: &'static str,
    pub device: &'static str,
    pub screens: &'static str,
    pub summary_heading: &'static str,
    pub rate_label: &'static str,
    pub rate_note: &'static str,
    pub not_available: &'static str,
    pub points_vs: &'static str,
    pub open_issues: &'static str,
    pub new_issues: &'static str,
    pub fixed_issues: &'static str,
    pub ignored_issues: &'static str,
    pub by_rule_heading: &'static str,
    pub col_problem: &'static str,
    pub col_open: &'static str,
    pub col_severity: &'static str,
    pub col_screens: &'static str,
    pub criteria_heading: &'static str,
    pub col_reference: &'static str,
    pub col_criterion: &'static str,
    pub col_level: &'static str,
    pub col_status: &'static str,
    pub col_issues: &'static str,
    pub non_normative: &'static str,
    pub screens_heading: &'static str,
    pub activity: &'static str,
    pub no_issue_on_screen: &'static str,
    pub screenshot_caption: &'static str,
    pub no_screenshot: &'static str,
    pub rules_not_verified: &'static str,
    pub element: &'static str,
    pub measure: &'static str,
    pub fix: &'static str,
    pub done_when: &'static str,
    pub issue_id: &'static str,
    pub likely_source: &'static str,
    pub confidence: &'static str,
    pub ignored: &'static str,
    pub fixed_heading: &'static str,
    pub no_baseline: &'static str,
    pub nothing_fixed: &'static str,
    pub not_rechecked_heading: &'static str,
    pub manual_heading: &'static str,
    pub manual_intro: &'static str,
    pub limitations_heading: &'static str,
    pub method_heading: &'static str,
    pub method_automated: &'static str,
    pub method_not_automated: &'static str,
    pub method_rules: &'static str,
    pub engine_version: &'static str,
    pub rules_digest: &'static str,
    pub digest_changed: &'static str,
    pub errors_heading: &'static str,
    pub ai_note: &'static str,
    pub colour_text: &'static str,
    pub colour_background: &'static str,
    pub colour_suggested: &'static str,
    pub baseline: &'static str,
    pub summary_line_rate: &'static str,
    pub summary_line_issues: &'static str,
    pub summary_line_report: &'static str,
}

pub static EN: Labels = Labels {
    colon: ":",
    report_title: "Accessibility report",
    skip_link: "Skip to summary",
    app: "App",
    build: "Build",
    captured_at: "Captured",
    device: "Device",
    screens: "Screens",
    summary_heading: "Summary",
    rate_label: "Automatic conformity rate",
    rate_note: "Share of the automatically testable criteria that pass. It is not a full conformity rate: the manual checks listed below remain.",
    not_available: "n/a",
    points_vs: "pts vs",
    open_issues: "Open issues",
    new_issues: "New",
    fixed_issues: "Fixed",
    ignored_issues: "ignored",
    by_rule_heading: "What to fix first",
    col_problem: "Problem",
    col_open: "Open issues",
    col_severity: "Severity",
    col_screens: "Screens",
    criteria_heading: "Criteria",
    col_reference: "Reference",
    col_criterion: "Criterion",
    col_level: "Level",
    col_status: "Status",
    col_issues: "Issues",
    non_normative: "guideline",
    screens_heading: "Screen by screen",
    activity: "Activity",
    no_issue_on_screen: "No automatic issue on this screen.",
    screenshot_caption: "Screenshot with the issues outlined; numbers match the list below.",
    no_screenshot: "No screenshot for this screen.",
    rules_not_verified: "Not verified without a screenshot",
    element: "Element",
    measure: "Measure",
    fix: "How to fix",
    done_when: "Done when",
    issue_id: "Id",
    likely_source: "Likely source",
    confidence: "confidence",
    ignored: "ignored",
    fixed_heading: "Fixed since the baseline",
    no_baseline: "No baseline report was provided.",
    nothing_fixed: "No issue was fixed since the baseline.",
    not_rechecked_heading: "Not rechecked (screen not captured this time)",
    manual_heading: "Manual checks",
    manual_intro: "These criteria cannot be verified automatically. Check them with TalkBack, an external keyboard and large fonts.",
    limitations_heading: "Known limitations",
    method_heading: "Method",
    method_automated: "Automated: accessible names of interactive elements, touch target sizes, text contrast measured on the screenshot, duplicated control names.",
    method_not_automated: "Not automated: everything that depends on meaning or interaction (reading order, label relevance, gestures, dynamic states).",
    method_rules: "Rules",
    engine_version: "Engine",
    rules_digest: "Rules digest",
    digest_changed: "The baseline was produced with different rules or settings.",
    errors_heading: "Screens that could not be analysed",
    ai_note: "Per-screen briefs for AI coding agents are in the ai/ folder next to this report.",
    colour_text: "text",
    colour_background: "background",
    colour_suggested: "suggested",
    baseline: "baseline",
    summary_line_rate: "Automatic conformity rate",
    summary_line_issues: "Issues",
    summary_line_report: "Report",
};

pub static FR: Labels = Labels {
    colon: " :",
    report_title: "Rapport d'accessibilité",
    skip_link: "Aller au résumé",
    app: "Application",
    build: "Build",
    captured_at: "Capture",
    device: "Appareil",
    screens: "Écrans",
    summary_heading: "Résumé",
    rate_label: "Taux de conformité automatique",
    rate_note: "Part des critères vérifiables automatiquement qui sont respectés. Ce n'est pas un taux de conformité complet : les vérifications manuelles listées plus bas restent à faire.",
    not_available: "n.d.",
    points_vs: "pts par rapport à",
    open_issues: "Problèmes ouverts",
    new_issues: "Nouveaux",
    fixed_issues: "Corrigés",
    ignored_issues: "ignorés",
    by_rule_heading: "Quoi corriger en premier",
    col_problem: "Problème",
    col_open: "Problèmes ouverts",
    col_severity: "Sévérité",
    col_screens: "Écrans",
    criteria_heading: "Critères",
    col_reference: "Référence",
    col_criterion: "Critère",
    col_level: "Niveau",
    col_status: "Statut",
    col_issues: "Écarts",
    non_normative: "recommandation",
    screens_heading: "Détail par écran",
    activity: "Activité",
    no_issue_on_screen: "Aucun problème automatique sur cet écran.",
    screenshot_caption: "Capture avec les zones fautives encadrées ; les numéros renvoient à la liste ci-dessous.",
    no_screenshot: "Pas de capture pour cet écran.",
    rules_not_verified: "Non vérifié sans capture",
    element: "Élément",
    measure: "Mesure",
    fix: "Piste de correction",
    done_when: "Corrigé quand",
    issue_id: "Identifiant",
    likely_source: "Source probable",
    confidence: "confiance",
    ignored: "ignoré",
    fixed_heading: "Corrigés depuis la référence",
    no_baseline: "Aucun rapport de référence n'a été fourni.",
    nothing_fixed: "Aucun problème corrigé depuis la référence.",
    not_rechecked_heading: "Non revérifiés (écran non capturé cette fois)",
    manual_heading: "Vérifications manuelles",
    manual_intro: "Ces critères ne sont pas vérifiables automatiquement. Les contrôler avec TalkBack, un clavier externe et une grande taille de police.",
    limitations_heading: "Limites connues",
    method_heading: "Méthode",
    method_automated: "Automatisé : noms accessibles des éléments interactifs, taille des cibles tactiles, contraste du texte mesuré sur la capture, noms de contrôles en double.",
    method_not_automated: "Non automatisé : tout ce qui dépend du sens ou de l'interaction (ordre de lecture, pertinence des libellés, gestes, états dynamiques).",
    method_rules: "Règles",
    engine_version: "Moteur",
    rules_digest: "Empreinte des règles",
    digest_changed: "La référence a été produite avec d'autres règles ou réglages.",
    errors_heading: "Écrans qui n'ont pas pu être analysés",
    ai_note: "Des fiches par écran destinées aux agents IA sont dans le dossier ai/ à côté de ce rapport.",
    colour_text: "texte",
    colour_background: "fond",
    colour_suggested: "suggestion",
    baseline: "référence",
    summary_line_rate: "Taux de conformité automatique",
    summary_line_issues: "Problèmes",
    summary_line_report: "Rapport",
};

pub fn labels(lang: Lang) -> &'static Labels {
    match lang {
        Lang::En => &EN,
        Lang::Fr => &FR,
    }
}

pub fn severity(s: Severity, lang: Lang) -> &'static str {
    match s {
        Severity::Critical => lang.pick("Critical", "Critique"),
        Severity::Major => lang.pick("Major", "Majeur"),
        Severity::Minor => lang.pick("Minor", "Mineur"),
        Severity::Info => lang.pick("Info", "Info"),
    }
}

pub fn confidence(c: Confidence, lang: Lang) -> &'static str {
    match c {
        Confidence::High => lang.pick("high", "haute"),
        Confidence::Medium => lang.pick("medium", "moyenne"),
        Confidence::Low => lang.pick("low", "faible"),
    }
}

pub fn status(s: Status, lang: Lang) -> &'static str {
    match s {
        Status::New => lang.pick("new", "nouveau"),
        Status::Existing => lang.pick("existing", "existant"),
    }
}

pub fn criterion_status(s: CriterionStatus, lang: Lang) -> &'static str {
    match s {
        CriterionStatus::Pass => lang.pick("Pass", "Conforme"),
        CriterionStatus::Fail => lang.pick("Fail", "Non conforme"),
        CriterionStatus::NotApplicable => lang.pick("Not applicable", "Non applicable"),
        CriterionStatus::Manual => lang.pick("Manual check", "Vérification manuelle"),
    }
}

/// Up to 2 decimals, trailing zeros removed, decimal comma in French.
pub fn number(x: f64, lang: Lang) -> String {
    let x = if x == 0.0 { 0.0 } else { x };
    let s = format!("{x:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    match lang {
        Lang::En => s,
        Lang::Fr => s.replace('.', ","),
    }
}

/// Change of the rate in percentage points, on the same 2-decimal values as the JSON
/// (no spurious "-0").
pub fn points_delta(now: f64, before: f64) -> f64 {
    let pts = ((a11y_report_core::num::round2(now) - a11y_report_core::num::round2(before))
        * 100.0)
        .round();
    if pts == 0.0 { 0.0 } else { pts }
}

pub fn percent(rate: f64, lang: Lang) -> String {
    let p = (rate * 100.0).round();
    match lang {
        Lang::En => format!("{p}%"),
        Lang::Fr => format!("{p} %"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(number(4.47, Lang::En), "4.47");
        assert_eq!(number(4.47, Lang::Fr), "4,47");
        assert_eq!(number(48.0, Lang::En), "48");
        assert_eq!(number(3.1, Lang::En), "3.1");
        assert_eq!(percent(0.75, Lang::Fr), "75 %");
        assert_eq!(percent(0.666, Lang::En), "67%");
        assert_eq!(number(-0.0, Lang::En), "0");
        assert_eq!(points_delta(0.6667, 0.67), 0.0);
        assert_eq!(points_delta(0.75, 0.5), 25.0);
    }
}
