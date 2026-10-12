//! Human-readable descriptions of findings: element, measure, fix and acceptance test.
//! Static texts per rule (MVP); LLM suggestions will live in a separate layer.

use crate::i18n::number;
use a11y_report_core::finding::MissingNameKind;
use a11y_report_core::model::simple_class;
use a11y_report_core::{ElementRef, Evidence, Finding, Lang};

/// `Button "Join" (#btn_join)`.
pub fn element(e: &ElementRef) -> String {
    let mut s = simple_class(&e.class).to_string();
    if let Some(name) = e.accessible_name.as_deref().or(e.text.as_deref()) {
        s.push_str(&format!(" \"{name}\""));
    }
    if let Some(id) = &e.resource_id {
        s.push_str(&format!(" (#{id})"));
    }
    s
}

pub fn measure(f: &Finding, lang: Lang) -> String {
    match &f.evidence {
        Evidence::MissingName { element_kind } => match element_kind {
            MissingNameKind::Image => lang.pick(
                "Interactive image with no content description and no text: TalkBack announces only \"button\" or \"unlabelled\".",
                "Image interactive sans description ni texte : TalkBack annonce seulement « bouton » ou « sans libellé ».",
            ),
            MissingNameKind::EditText => lang.pick(
                "Text field with no text and no content description in the dump (a hint or labelFor may exist but is not visible to the engine).",
                "Champ de saisie sans texte ni description dans le dump (un hint ou labelFor peut exister mais n'est pas visible par le moteur).",
            ),
            MissingNameKind::Control => lang.pick(
                "Interactive element with no content description, no text and no named child.",
                "Élément interactif sans description, sans texte et sans enfant nommé.",
            ),
        }
        .to_string(),
        Evidence::TargetSize {
            width_dp,
            height_dp,
            min_dp,
        } => format!(
            "{} × {} dp ({} {} dp)",
            number(*width_dp, lang),
            number(*height_dp, lang),
            lang.pick("minimum", "minimum"),
            number(*min_dp, lang)
        ),
        Evidence::Contrast {
            ratio,
            required,
            fg,
            bg,
            text_height_dp,
            large_text,
            background_uniformity,
            ..
        } => {
            let mut s = format!(
                "{} {}:1 ({} {}:1) — {} {fg} {} {bg}, {} ≈ {} dp{}",
                lang.pick("Contrast", "Contraste"),
                number(*ratio, lang),
                lang.pick("required", "requis"),
                number(*required, lang),
                lang.pick("text", "texte"),
                lang.pick("on", "sur"),
                lang.pick("text height", "hauteur du texte"),
                number(*text_height_dp, lang),
                if *large_text {
                    lang.pick(" (large text)", " (grand texte)")
                } else {
                    ""
                }
            );
            if *background_uniformity < 0.6 {
                s.push_str(lang.pick(
                    ". Busy background (image or gradient): check by eye.",
                    ". Fond chargé (image ou dégradé) : vérifier à l'œil.",
                ));
            }
            s
        }
        Evidence::DuplicateName { name, others } => format!(
            "{} \"{name}\" {} {} {}",
            lang.pick("The name", "Le nom"),
            lang.pick("is shared by", "est porté par"),
            others.len() + 1,
            lang.pick("controls on this screen.", "contrôles sur cet écran.")
        ),
    }
}

pub fn fix(f: &Finding, lang: Lang) -> String {
    match &f.evidence {
        Evidence::MissingName { element_kind } => match element_kind {
            MissingNameKind::Image => lang.pick(
                "Add a content description that says what the button does (Views: android:contentDescription=\"@string/…\"; Compose: Icon(contentDescription = stringResource(…))). Describe the action, not the icon.",
                "Ajouter une description qui dit ce que fait le bouton (Views : android:contentDescription=\"@string/…\" ; Compose : Icon(contentDescription = stringResource(…))). Décrire l'action, pas l'icône.",
            ),
            MissingNameKind::EditText => lang.pick(
                "Give the field a persistent label: a visible TextView with android:labelFor, a TextInputLayout hint, or Compose TextField(label = { Text(…) }).",
                "Donner au champ une étiquette persistante : un TextView visible avec android:labelFor, un hint de TextInputLayout, ou TextField(label = { Text(…) }) en Compose.",
            ),
            MissingNameKind::Control => lang.pick(
                "Give the control a name: visible text, android:contentDescription, or in Compose Modifier.semantics { contentDescription = … } on the clickable element (or merge its children with Modifier.semantics(mergeDescendants = true)).",
                "Donner un nom au contrôle : texte visible, android:contentDescription, ou en Compose Modifier.semantics { contentDescription = … } sur l'élément cliquable (ou fusionner ses enfants avec Modifier.semantics(mergeDescendants = true)).",
            ),
        }
        .to_string(),
        Evidence::TargetSize { min_dp, .. } => format!(
            "{} {} dp {}",
            lang.pick(
                "Enlarge the touch area to at least",
                "Agrandir la zone tactile à au moins"
            ),
            number(*min_dp, lang),
            lang.pick(
                "without changing the visual size: Views: android:minWidth/minHeight or padding (TouchDelegate as a last resort); Compose: Modifier.minimumInteractiveComponentSize() or Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp).",
                "sans changer la taille visuelle : Views : android:minWidth/minHeight ou padding (TouchDelegate en dernier recours) ; Compose : Modifier.minimumInteractiveComponentSize() ou Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp)."
            )
        ),
        Evidence::Contrast {
            fg,
            bg,
            suggested_fg,
            suggested_ratio,
            ..
        } => {
            let base = lang.pick(
                "Darken or lighten the text colour (or the background) in the theme/colour resource rather than on this element only.",
                "Foncer ou éclaircir la couleur du texte (ou du fond) dans le thème ou la ressource de couleur plutôt que sur ce seul élément.",
            );
            match (suggested_fg, suggested_ratio) {
                (Some(s), Some(r)) => format!(
                    "{} {s} {} {fg} ({} {}:1 {} {bg}). {base}",
                    lang.pick("Use", "Utiliser"),
                    lang.pick("instead of", "au lieu de"),
                    lang.pick("ratio", "ratio"),
                    number(*r, lang),
                    lang.pick("on", "sur"),
                ),
                _ => base.to_string(),
            }
        }
        Evidence::DuplicateName { .. } => lang.pick(
            "Make each name unique by adding context, e.g. \"Water Monstera\" instead of \"Water\" (Views: contentDescription built with the item name; Compose: Modifier.semantics { contentDescription = \"Water $plantName\" }).",
            "Rendre chaque nom unique en ajoutant le contexte, par ex. « Arroser Monstera » au lieu de « Arroser » (Views : contentDescription construite avec le nom de l'élément ; Compose : Modifier.semantics { contentDescription = \"Arroser $nomPlante\" }).",
        )
        .to_string(),
    }
}

pub fn done_when(f: &Finding, lang: Lang) -> String {
    match &f.evidence {
        Evidence::MissingName { .. } => lang
            .pick(
                "TalkBack announces a meaningful name for this element and this issue id disappears from the next report.",
                "TalkBack annonce un nom pertinent pour cet élément et cet identifiant disparaît du prochain rapport.",
            )
            .to_string(),
        Evidence::TargetSize { min_dp, .. } => format!(
            "{} {} dp.",
            lang.pick(
                "Both sides of the element's bounds are at least",
                "Les deux côtés de l'élément mesurent au moins"
            ),
            number(*min_dp, lang)
        ),
        Evidence::Contrast { required, .. } => format!(
            "{} {}:1.",
            lang.pick(
                "The measured contrast is at least",
                "Le contraste mesuré est d'au moins"
            ),
            number(*required, lang)
        ),
        Evidence::DuplicateName { .. } => lang
            .pick(
                "No two interactive elements on the screen share the same accessible name.",
                "Aucun élément interactif de l'écran ne partage le même nom accessible.",
            )
            .to_string(),
    }
}

/// Generic fix recipe per rule, for the AI briefs.
pub fn recipe(rule: &str, lang: Lang) -> &'static str {
    match rule {
        "missing-name" => lang.pick(
            "- Views: set `android:contentDescription=\"@string/...\"` on ImageButton/ImageView that act as buttons; for custom clickable containers, make sure a child TextView carries the label or set the description on the container.\n- Compose: `Icon(imageVector, contentDescription = stringResource(R.string.x))`; for `Modifier.clickable` rows, merge children with `Modifier.semantics(mergeDescendants = true) {}` or set `contentDescription` in `semantics {}`.\n- Purely decorative images must not be clickable; give them `contentDescription = null` / `importantForAccessibility=\"no\"`.\n- Put every label in `strings.xml` (translatable).",
            "- Views : `android:contentDescription=\"@string/...\"` sur les ImageButton/ImageView qui servent de boutons ; pour un conteneur cliquable personnalisé, s'assurer qu'un TextView enfant porte le libellé ou mettre la description sur le conteneur.\n- Compose : `Icon(imageVector, contentDescription = stringResource(R.string.x))` ; pour une ligne `Modifier.clickable`, fusionner les enfants avec `Modifier.semantics(mergeDescendants = true) {}` ou définir `contentDescription` dans `semantics {}`.\n- Une image purement décorative ne doit pas être cliquable : `contentDescription = null` / `importantForAccessibility=\"no\"`.\n- Mettre chaque libellé dans `strings.xml` (traduisible).",
        ),
        "target-size" => lang.pick(
            "- Keep the visual size, enlarge the touch area.\n- Views: `android:minWidth=\"48dp\"` / `android:minHeight=\"48dp\"`, or padding; Material buttons already enforce 48dp unless `minHeight` was overridden or `insetTop/insetBottom` set to 0.\n- Compose: Material components apply `minimumInteractiveComponentSize()`; for custom clickables use `Modifier.minimumInteractiveComponentSize()` or `Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp)` before `clickable`.",
            "- Garder la taille visuelle, agrandir la zone tactile.\n- Views : `android:minWidth=\"48dp\"` / `android:minHeight=\"48dp\"`, ou du padding ; les boutons Material imposent déjà 48dp sauf si `minHeight` a été surchargé ou `insetTop/insetBottom` mis à 0.\n- Compose : les composants Material appliquent `minimumInteractiveComponentSize()` ; pour un clickable personnalisé, utiliser `Modifier.minimumInteractiveComponentSize()` ou `Modifier.sizeIn(minWidth = 48.dp, minHeight = 48.dp)` avant `clickable`.",
        ),
        "contrast-text" => lang.pick(
            "- Fix the colour where it is defined (theme attribute, `colors.xml`, Compose `ColorScheme`), not on a single view, unless the colour is local.\n- Normal text needs 4.5:1, large text (≥ 18pt, about 24dp) needs 3:1.\n- Use the suggested colour from the evidence when present: same hue, adjusted lightness.\n- Check the dark theme too: the same token often fails there.",
            "- Corriger la couleur là où elle est définie (attribut de thème, `colors.xml`, `ColorScheme` Compose), pas sur une seule vue, sauf si la couleur est locale.\n- Le texte normal demande 4,5:1, le grand texte (≥ 18 pt, environ 24 dp) 3:1.\n- Utiliser la couleur suggérée dans la mesure quand elle existe : même teinte, luminosité ajustée.\n- Vérifier aussi le thème sombre : le même jeton y échoue souvent.",
        ),
        "duplicate-name" => lang.pick(
            "- Typical case: the same \"Join\"/\"Delete\" button repeated in list items. Add the item's name to the accessible name only (the visible text can stay short).\n- Views: `button.contentDescription = getString(R.string.water_plant, plant.name)`.\n- Compose: `Modifier.semantics { contentDescription = \"…${item.name}\" }` on the button, or merge the row so the item name is read with the action.",
            "- Cas typique : le même bouton « Rejoindre »/« Supprimer » répété dans les éléments d'une liste. Ajouter le nom de l'élément au seul nom accessible (le texte visible peut rester court).\n- Views : `button.contentDescription = getString(R.string.water_plant, plant.name)`.\n- Compose : `Modifier.semantics { contentDescription = \"…${item.name}\" }` sur le bouton, ou fusionner la ligne pour que le nom de l'élément soit lu avec l'action.",
        ),
        _ => "",
    }
}
