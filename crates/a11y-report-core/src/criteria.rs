//! Static catalog of the success criteria the engine knows about, automatable or not.
//!
//! One entry per WCAG success criterion, carrying its EN 301 549 v3.2.1 clause for native
//! software (chapter 11) when one exists. WCAG 2.5.8 is a WCAG 2.2 criterion: EN 301 549
//! v3.2.1 is based on WCAG 2.1 and has no clause for it, so none is invented.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Output language of human-readable strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    Fr,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
        }
    }

    pub fn pick(self, en: &'static str, fr: &'static str) -> &'static str {
        match self {
            Lang::En => en,
            Lang::Fr => fr,
        }
    }
}

pub type CriterionId = &'static str;

pub const WCAG_1_1_1: CriterionId = "wcag-1.1.1";
pub const WCAG_1_4_3: CriterionId = "wcag-1.4.3";
pub const WCAG_2_4_6: CriterionId = "wcag-2.4.6";
pub const WCAG_2_5_8: CriterionId = "wcag-2.5.8";
pub const WCAG_4_1_2: CriterionId = "wcag-4.1.2";
/// Android touch-target guideline (48 dp). Good practice, not a normative requirement.
pub const ANDROID_TOUCH_TARGET: CriterionId = "android-touch-target";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Criterion {
    pub id: CriterionId,
    /// WCAG 2.2 success criterion number.
    pub wcag: Option<&'static str>,
    /// EN 301 549 v3.2.1 clause (chapter 11, native software).
    pub en301549: Option<&'static str>,
    /// `A`, `AA`, or `-` for non-normative guidance.
    pub level: &'static str,
    pub title_en: &'static str,
    pub title_fr: &'static str,
    /// Some part of the criterion is measured by a rule.
    pub automatable: bool,
    /// Counts in the automatic conformity rate.
    pub normative: bool,
    /// What remains to check by hand (listed in `manual_checks`).
    pub manual_en: Option<&'static str>,
    pub manual_fr: Option<&'static str>,
}

impl Criterion {
    pub fn title(&self, lang: Lang) -> &'static str {
        lang.pick(self.title_en, self.title_fr)
    }

    pub fn manual_note(&self, lang: Lang) -> Option<&'static str> {
        match lang {
            Lang::En => self.manual_en,
            Lang::Fr => self.manual_fr,
        }
    }

    /// Human label such as `WCAG 1.4.3 · EN 301 549 11.1.4.3`.
    pub fn reference(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = self.wcag {
            parts.push(format!("WCAG {w}"));
        }
        if let Some(e) = self.en301549 {
            parts.push(format!("EN 301 549 {e}"));
        }
        if parts.is_empty() {
            parts.push("Android".to_string());
        }
        parts.join(" · ")
    }
}

macro_rules! crit {
    ($id:expr, $wcag:expr, $en:expr, $level:expr, $en_title:expr, $fr_title:expr, auto, $manual_en:expr, $manual_fr:expr) => {
        Criterion {
            id: $id,
            wcag: $wcag,
            en301549: $en,
            level: $level,
            title_en: $en_title,
            title_fr: $fr_title,
            automatable: true,
            normative: true,
            manual_en: $manual_en,
            manual_fr: $manual_fr,
        }
    };
    ($id:expr, $wcag:expr, $en:expr, $level:expr, $en_title:expr, $fr_title:expr, manual, $manual_en:expr, $manual_fr:expr) => {
        Criterion {
            id: $id,
            wcag: $wcag,
            en301549: $en,
            level: $level,
            title_en: $en_title,
            title_fr: $fr_title,
            automatable: false,
            normative: true,
            manual_en: Some($manual_en),
            manual_fr: Some($manual_fr),
        }
    };
}

/// Sorted by WCAG number, the non-normative Android guideline last.
pub static CATALOG: &[Criterion] = &[
    crit!(
        WCAG_1_1_1,
        Some("1.1.1"),
        Some("11.1.1.1"),
        "A",
        "Non-text content",
        "Contenu non textuel",
        auto,
        Some(
            "Non-interactive images without a description may be decorative or informative: check that informative images are described and decorative ones are hidden from TalkBack."
        ),
        Some(
            "Les images non interactives sans description peuvent être décoratives ou porteuses d'information : vérifier que les images informatives sont décrites et que les images décoratives sont masquées à TalkBack."
        )
    ),
    crit!(
        "wcag-1.2.2",
        Some("1.2.2"),
        Some("11.1.2.2"),
        "A",
        "Captions (prerecorded)",
        "Sous-titres (pré-enregistrés)",
        manual,
        "Videos with sound need synchronized captions.",
        "Les vidéos avec du son doivent avoir des sous-titres synchronisés."
    ),
    crit!(
        "wcag-1.3.1",
        Some("1.3.1"),
        Some("11.1.3.1"),
        "A",
        "Info and relationships",
        "Information et relations",
        manual,
        "Headings, lists and groups must be exposed semantically (e.g. heading semantics, collection info), not only visually.",
        "Les titres, listes et regroupements doivent être exposés sémantiquement (titres, collections), pas seulement visuellement."
    ),
    crit!(
        "wcag-1.3.2",
        Some("1.3.2"),
        Some("11.1.3.2"),
        "A",
        "Meaningful sequence",
        "Ordre séquentiel logique",
        manual,
        "Reading order depends on meaning, not only on position: swipe through the screen with TalkBack.",
        "L'ordre de lecture dépend du sens, pas seulement de la position : parcourir l'écran au balayage avec TalkBack."
    ),
    crit!(
        "wcag-1.3.4",
        Some("1.3.4"),
        Some("11.1.3.4"),
        "AA",
        "Orientation",
        "Orientation",
        manual,
        "The app must work in portrait and landscape unless an orientation is essential.",
        "L'application doit fonctionner en portrait et en paysage, sauf si une orientation est essentielle."
    ),
    crit!(
        "wcag-1.4.1",
        Some("1.4.1"),
        Some("11.1.4.1"),
        "A",
        "Use of color",
        "Utilisation de la couleur",
        manual,
        "Information must not be conveyed by colour alone (errors, states, charts).",
        "L'information ne doit pas être véhiculée uniquement par la couleur (erreurs, états, graphiques)."
    ),
    crit!(
        WCAG_1_4_3,
        Some("1.4.3"),
        Some("11.1.4.3"),
        "AA",
        "Contrast (minimum)",
        "Contraste (minimum)",
        auto,
        Some(
            "Text over images or gradients is reported with low confidence and must be checked by eye."
        ),
        Some(
            "Le texte sur image ou dégradé est signalé avec une confiance faible et doit être vérifié à l'œil."
        )
    ),
    crit!(
        "wcag-1.4.4",
        Some("1.4.4"),
        Some("11.1.4.4"),
        "AA",
        "Resize text",
        "Redimensionnement du texte",
        manual,
        "Set the system font size to 200% and check that no text is truncated or overlapping.",
        "Régler la taille de police système à 200 % et vérifier qu'aucun texte n'est tronqué ou superposé."
    ),
    crit!(
        "wcag-1.4.11",
        Some("1.4.11"),
        Some("11.1.4.11"),
        "AA",
        "Non-text contrast",
        "Contraste du contenu non textuel",
        manual,
        "Icons, input borders and focus indicators need a 3:1 contrast with their surroundings.",
        "Les icônes, bordures de champs et indicateurs de focus doivent avoir un contraste de 3:1 avec leur environnement."
    ),
    crit!(
        "wcag-2.1.1",
        Some("2.1.1"),
        Some("11.2.1.1"),
        "A",
        "Keyboard",
        "Clavier",
        manual,
        "Every action must be reachable with an external keyboard or Switch Access.",
        "Chaque action doit être accessible avec un clavier externe ou Switch Access."
    ),
    crit!(
        "wcag-2.2.2",
        Some("2.2.2"),
        Some("11.2.2.2"),
        "A",
        "Pause, stop, hide",
        "Mettre en pause, arrêter, masquer",
        manual,
        "Carousels and animations that last more than 5 seconds must be pausable.",
        "Les carrousels et animations de plus de 5 secondes doivent pouvoir être mis en pause."
    ),
    crit!(
        "wcag-2.4.3",
        Some("2.4.3"),
        Some("11.2.4.3"),
        "A",
        "Focus order",
        "Parcours du focus",
        manual,
        "Focus must move in an order that preserves meaning, including after dialogs open and close.",
        "Le focus doit suivre un ordre qui préserve le sens, y compris à l'ouverture et à la fermeture des dialogues."
    ),
    crit!(
        WCAG_2_4_6,
        Some("2.4.6"),
        Some("11.2.4.6"),
        "AA",
        "Headings and labels",
        "En-têtes et étiquettes",
        auto,
        Some(
            "Only duplicated labels are detected: whether each label describes its purpose needs human judgement."
        ),
        Some(
            "Seuls les libellés en double sont détectés : la pertinence de chaque libellé demande un jugement humain."
        )
    ),
    crit!(
        "wcag-2.4.7",
        Some("2.4.7"),
        Some("11.2.4.7"),
        "AA",
        "Focus visible",
        "Visibilité du focus",
        manual,
        "With a keyboard, the focused element must be visibly highlighted.",
        "Au clavier, l'élément qui a le focus doit être visiblement mis en évidence."
    ),
    crit!(
        "wcag-2.5.1",
        Some("2.5.1"),
        Some("11.2.5.1"),
        "A",
        "Pointer gestures",
        "Gestes de pointage",
        manual,
        "Multi-finger or path-based gestures need a single-tap alternative.",
        "Les gestes à plusieurs doigts ou à tracé doivent avoir une alternative en simple appui."
    ),
    crit!(
        "wcag-2.5.3",
        Some("2.5.3"),
        Some("11.2.5.3"),
        "A",
        "Label in name",
        "Étiquette dans le nom",
        manual,
        "The accessible name must contain the visible label (voice control users say what they see).",
        "Le nom accessible doit contenir le libellé visible (la commande vocale utilise ce qui est affiché)."
    ),
    crit!(
        WCAG_2_5_8,
        Some("2.5.8"),
        None,
        "AA",
        "Target size (minimum)",
        "Taille de la cible (minimum)",
        auto,
        Some("The spacing exception (small targets far enough from others) is not evaluated."),
        Some(
            "L'exception d'espacement (petites cibles suffisamment éloignées des autres) n'est pas évaluée."
        )
    ),
    crit!(
        "wcag-3.3.1",
        Some("3.3.1"),
        Some("11.3.3.1"),
        "A",
        "Error identification",
        "Identification des erreurs",
        manual,
        "Form errors must be described in text and announced to TalkBack.",
        "Les erreurs de formulaire doivent être décrites en texte et annoncées à TalkBack."
    ),
    crit!(
        "wcag-3.3.2",
        Some("3.3.2"),
        Some("11.3.3.2"),
        "A",
        "Labels or instructions",
        "Étiquettes ou instructions",
        manual,
        "Input fields need a persistent label, not only a placeholder that disappears.",
        "Les champs de saisie doivent avoir une étiquette persistante, pas seulement un texte indicatif qui disparaît."
    ),
    crit!(
        WCAG_4_1_2,
        Some("4.1.2"),
        Some("11.4.1.2"),
        "A",
        "Name, role, value",
        "Nom, rôle et valeur",
        auto,
        Some("Roles and states (selected, expanded, checked) must be checked with TalkBack."),
        Some(
            "Les rôles et états (sélectionné, déplié, coché) doivent être vérifiés avec TalkBack."
        )
    ),
    crit!(
        "wcag-4.1.3",
        Some("4.1.3"),
        Some("11.4.1.3"),
        "AA",
        "Status messages",
        "Messages d'état",
        manual,
        "Snackbars, toasts and loading states must be announced without moving focus.",
        "Les snackbars, toasts et états de chargement doivent être annoncés sans déplacer le focus."
    ),
    Criterion {
        id: ANDROID_TOUCH_TARGET,
        wcag: None,
        en301549: None,
        level: "-",
        title_en: "Touch target size (Android 48 dp guideline)",
        title_fr: "Taille des cibles tactiles (recommandation Android 48 dp)",
        automatable: true,
        normative: false,
        manual_en: None,
        manual_fr: None,
    },
];

pub fn criterion(id: &str) -> Option<&'static Criterion> {
    CATALOG.iter().find(|c| c.id == id)
}

/// Limitations of the automatic audit, reported verbatim in every report.
pub static KNOWN_LIMITATIONS: &[(&str, &str)] = &[
    (
        "WCAG 2.5.8: the spacing exception is not evaluated.",
        "WCAG 2.5.8 : l'exception d'espacement n'est pas évaluée.",
    ),
    (
        "Contrast: bold text cannot be detected, so the 14 pt bold large-text rule is not applied.",
        "Contraste : le gras n'est pas détectable, la règle du grand texte à 14 pt en gras ne s'applique donc pas.",
    ),
    (
        "Contrast: text over images or gradients is reported with low confidence and never fails a criterion.",
        "Contraste : le texte sur image ou dégradé est signalé avec une confiance faible et ne fait jamais échouer un critère.",
    ),
    (
        "Missing names on text fields have low confidence: hint and labelFor are not visible in uiautomator dumps.",
        "Les noms manquants sur les champs de saisie ont une confiance faible : hint et labelFor n'apparaissent pas dans les dumps uiautomator.",
    ),
    (
        "Only captured screens are audited; screens that were not captured are not checked.",
        "Seuls les écrans capturés sont audités ; un écran non capturé n'est pas vérifié.",
    ),
    (
        "EN 301 549 references use the chapter 11 clause matching each WCAG criterion.",
        "Les références EN 301 549 utilisent la clause du chapitre 11 correspondant à chaque critère WCAG.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_are_unique_and_rule_criteria_exist() {
        let ids: BTreeSet<_> = CATALOG.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), CATALOG.len());
        for id in [
            WCAG_1_1_1,
            WCAG_1_4_3,
            WCAG_2_4_6,
            WCAG_2_5_8,
            WCAG_4_1_2,
            ANDROID_TOUCH_TARGET,
        ] {
            assert!(criterion(id).is_some_and(|c| c.automatable), "{id}");
        }
    }

    #[test]
    fn no_en_mapping_for_2_5_8() {
        assert_eq!(criterion(WCAG_2_5_8).and_then(|c| c.en301549), None);
        assert_eq!(
            criterion(WCAG_1_4_3).map(|c| c.reference()).as_deref(),
            Some("WCAG 1.4.3 · EN 301 549 11.1.4.3")
        );
    }

    #[test]
    fn manual_criteria_explain_why() {
        for c in CATALOG.iter().filter(|c| !c.automatable) {
            assert!(c.manual_en.is_some() && c.manual_fr.is_some(), "{}", c.id);
        }
    }
}
