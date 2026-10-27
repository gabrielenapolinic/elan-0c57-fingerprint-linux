// SPDX-License-Identifier: LGPL-2.1-or-later

//! Messages in the language of the system.
//!
//! English is the source language; Italian, French, German, Spanish and
//! Portuguese follow the session locale (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`,
//! `LANG`), else the system one (`/etc/locale.conf`); any other language
//! falls back to English. `ELAN_LANG` forces a language.

use std::sync::OnceLock;

fn detect() -> String {
    if let Ok(forced) = std::env::var("ELAN_LANG") {
        return forced;
    }
    let mut locale = String::new();
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        let value = std::env::var(var).unwrap_or_default();
        if !value.is_empty() && value != "C" && value != "POSIX" && !value.starts_with("C.") {
            let language = std::env::var("LANGUAGE").unwrap_or_default();
            locale = language.split(':').next().filter(|l| !l.is_empty()).map_or(value, str::to_owned);
            break;
        }
    }
    if locale.is_empty() {
        let conf = std::fs::read_to_string("/etc/locale.conf").unwrap_or_default();
        if let Some(value) = conf.lines().find_map(|l| l.strip_prefix("LANG=")) {
            locale = value.trim_matches('"').to_owned();
        }
    }
    locale.split(['_', '.', '@', '-']).next().unwrap_or("en").to_lowercase()
}

/// Language code in use (en, it, fr, de, es, pt…).
pub fn lang() -> &'static str {
    static LANG: OnceLock<String> = OnceLock::new();
    LANG.get_or_init(detect)
}

/// Translation of `msgid` (English), or `msgid` itself.
pub fn tr(msgid: &'static str) -> &'static str {
    let table: &[(&str, &str)] = match lang() {
        "it" => IT,
        "fr" => FR,
        "de" => DE,
        "es" => ES,
        "pt" => PT,
        _ => return msgid,
    };
    table.iter().find(|(id, _)| *id == msgid).map_or(msgid, |(_, text)| text)
}

/// [`tr`], then replaces each `{name}` with its value.
pub fn trf(msgid: &'static str, values: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut text = tr(msgid).to_owned();
    for (name, value) in values {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

/// Translates clap's help: headings, the help and version flags, and the
/// usage line, for the command and all its subcommands.
pub fn localize(cmd: clap::Command) -> clap::Command {
    let heading = |id: &'static str| tr(id).trim_end_matches(':').trim_end();
    let template = format!("{{about-with-newline}}\n{} {{usage}}\n\n{{all-args}}{{after-help}}", tr("Usage:"));
    let has_version = cmd.get_version().is_some();
    let mut cmd = cmd
        .help_template(template)
        .subcommand_help_heading(heading("Commands:"))
        .disable_help_flag(true)
        .mut_args(|a| {
            let title = if a.is_positional() { heading("Arguments:") } else { heading("Options:") };
            a.help_heading(title)
        })
        .arg(
            clap::Arg::new("help")
                .short('h')
                .long("help")
                .action(clap::ArgAction::Help)
                .help(tr("Print help"))
                .help_heading(heading("Options:")),
        )
        .mut_subcommands(localize);
    if has_version {
        cmd = cmd.disable_version_flag(true).arg(
            clap::Arg::new("version")
                .short('V')
                .long("version")
                .action(clap::ArgAction::Version)
                .help(tr("Print version"))
                .help_heading(heading("Options:")),
        );
    }
    cmd.disable_help_subcommand(true)
}

/// Every translated message, for the tests.
pub const TABLES: [(&str, &[(&str, &str)]); 5] = [("it", IT), ("fr", FR), ("de", DE), ("es", ES), ("pt", PT)];

const IT: &[(&str, &str)] = &[
    // device errors
    ("the sensor did not answer in time", "il sensore non ha risposto in tempo"),
    ("unexpected reply from the sensor: {msg}", "risposta inattesa dal sensore: {msg}"),
    ("the image is darker than the background: was the finger on the sensor during calibration?",
     "l'immagine è più scura dello sfondo: il dito era sul sensore durante la calibrazione?"),
    ("calibration failed", "calibrazione fallita"),
    ("cancelled", "annullato"),
    ("USB error: {msg}", "errore USB: {msg}"),
    ("no 04f3:0c57 reader found, or no permission to open it (install udev/71-elan-0c57-uaccess.rules)",
     "nessun lettore 04f3:0c57 trovato, o mancano i permessi per aprirlo (installa udev/71-elan-0c57-uaccess.rules)"),
    ("the reader is in use (by fprintd?)", "il lettore è in uso (da fprintd?)"),
    // elan-capture
    ("Open source tool for the ELAN 04f3:0c57 fingerprint reader", "Strumento open source per il lettore di impronte ELAN 04f3:0c57"),
    ("Show firmware, sensor size and calibration values", "Mostra firmware, dimensioni del sensore e valori di calibrazione"),
    ("Switch the LED on for a moment, to check that commands reach the reader", "Accende il LED per un momento, per controllare che i comandi arrivino al lettore"),
    ("How long to keep it on, in seconds", "Per quanto tenerlo acceso, in secondi"),
    ("Capture touches of one finger into a dataset folder", "Cattura i tocchi di un dito in una cartella di dataset"),
    ("Finger label, used as sub-folder (e.g. right-index)", "Nome del dito, usato come sottocartella (es. right-index)"),
    ("Number of touches to capture", "Numero di tocchi da catturare"),
    ("Dataset folder", "Cartella del dataset"),
    ("Seconds to wait for each touch", "Secondi di attesa per ogni tocco"),
    ("Also save touches that cover only part of the sensor", "Salva anche i tocchi che coprono solo una parte del sensore"),
    ("firmware:", "firmware:"),
    ("sensor size:", "dimensioni del sensore:"),
    ("calibration mean:", "media di calibrazione:"),
    ("background mean:", "media dello sfondo:"),
    ("(after calibration)", "(dopo la calibrazione)"),
    ("use letters, digits, '-' or '_' for the finger name", "usa lettere, cifre, '-' o '_' per il nome del dito"),
    ("touch the sensor with {finger}…", "tocca il sensore con {finger}…"),
    ("partial touch ({pct}% of the sensor): press the whole fingertip, try again",
     "tocco parziale ({pct}% del sensore): appoggia tutto il polpastrello e riprova"),
    ("saved {path}", "salvato {path}"),
    ("no finger, stopping", "nessun dito, mi fermo"),
    ("{error} — lift the finger and try again", "{error} — solleva il dito e riprova"),
    ("lift the finger", "solleva il dito"),
    ("{n} touch(es) saved in {path}", "{n} tocchi salvati in {path}"),
    // elan-match
    ("Evaluate the fingerprint matcher on a dataset", "Valuta l'algoritmo di confronto delle impronte su un dataset"),
    ("Print the scores of split 0 (first touches enrolled) as CSV, in the format of research/reference.py",
     "Stampa i punteggi della suddivisione 0 (primi tocchi registrati) in CSV, nel formato di research/reference.py"),
    ("Time of one verification (one probe against a template), on one core",
     "Tempo di una verifica (un tocco contro un modello)"),
    ("Error rates over random splits of enrolled / verification touches",
     "Tassi di errore su suddivisioni casuali tra tocchi registrati e di verifica"),
    ("Also use partial touches (the driver would ask to repeat them)", "Usa anche i tocchi parziali (il driver li farebbe ripetere)"),
    ("{finger}: {n} partial touch(es) left out", "{finger}: {n} tocchi parziali esclusi"),
    ("genuine attempts accepted at the strictest threshold that rejects every impostor",
     "tentativi del dito giusto accettati con la soglia più severa che respinge ogni dito sbagliato"),
    ("fixed threshold {t}: genuine accepted {g}%, impostors accepted {i}% (highest impostor score {w})",
     "soglia fissa {t}: dito giusto accettato {g}%, dita sbagliate accettate {i}% (punteggio più alto di un dito sbagliato {w})"),
    // clap
    ("Usage:", "Uso:"),
    ("Commands:", "Comandi:"),
    ("Options:", "Opzioni:"),
    ("Arguments:", "Argomenti:"),
    ("Print help", "Mostra l'aiuto"),
    ("Print version", "Mostra la versione"),
];

const FR: &[(&str, &str)] = &[
    ("the sensor did not answer in time", "le capteur n'a pas répondu à temps"),
    ("unexpected reply from the sensor: {msg}", "réponse inattendue du capteur : {msg}"),
    ("the image is darker than the background: was the finger on the sensor during calibration?",
     "l'image est plus sombre que le fond : le doigt était-il sur le capteur pendant l'étalonnage ?"),
    ("calibration failed", "échec de l'étalonnage"),
    ("cancelled", "annulé"),
    ("USB error: {msg}", "erreur USB : {msg}"),
    ("no 04f3:0c57 reader found, or no permission to open it (install udev/71-elan-0c57-uaccess.rules)",
     "aucun lecteur 04f3:0c57 trouvé, ou pas le droit de l'ouvrir (installez udev/71-elan-0c57-uaccess.rules)"),
    ("the reader is in use (by fprintd?)", "le lecteur est utilisé (par fprintd ?)"),
    ("Open source tool for the ELAN 04f3:0c57 fingerprint reader", "Outil libre pour le lecteur d'empreintes ELAN 04f3:0c57"),
    ("Show firmware, sensor size and calibration values", "Affiche le firmware, la taille du capteur et l'étalonnage"),
    ("Switch the LED on for a moment, to check that commands reach the reader", "Allume la LED un instant, pour vérifier que les commandes arrivent au lecteur"),
    ("How long to keep it on, in seconds", "Durée d'allumage, en secondes"),
    ("Capture touches of one finger into a dataset folder", "Capture les contacts d'un doigt dans un dossier de données"),
    ("Finger label, used as sub-folder (e.g. right-index)", "Nom du doigt, utilisé comme sous-dossier (ex. right-index)"),
    ("Number of touches to capture", "Nombre de contacts à capturer"),
    ("Dataset folder", "Dossier des données"),
    ("Seconds to wait for each touch", "Secondes d'attente pour chaque contact"),
    ("Also save touches that cover only part of the sensor", "Enregistre aussi les contacts qui ne couvrent qu'une partie du capteur"),
    ("firmware:", "firmware :"),
    ("sensor size:", "taille du capteur :"),
    ("calibration mean:", "moyenne d'étalonnage :"),
    ("background mean:", "moyenne du fond :"),
    ("(after calibration)", "(après étalonnage)"),
    ("use letters, digits, '-' or '_' for the finger name", "utilisez des lettres, chiffres, '-' ou '_' pour le nom du doigt"),
    ("touch the sensor with {finger}…", "touchez le capteur avec {finger}…"),
    ("partial touch ({pct}% of the sensor): press the whole fingertip, try again",
     "contact partiel ({pct} % du capteur) : posez toute la pulpe du doigt et recommencez"),
    ("saved {path}", "enregistré {path}"),
    ("no finger, stopping", "pas de doigt, arrêt"),
    ("{error} — lift the finger and try again", "{error} — levez le doigt et recommencez"),
    ("lift the finger", "levez le doigt"),
    ("{n} touch(es) saved in {path}", "{n} contact(s) enregistré(s) dans {path}"),
    ("Evaluate the fingerprint matcher on a dataset", "Évalue l'algorithme de comparaison sur un jeu de données"),
    ("Print the scores of split 0 (first touches enrolled) as CSV, in the format of research/reference.py",
     "Affiche les scores du découpage 0 (premiers contacts enregistrés) en CSV, au format de research/reference.py"),
    ("Time of one verification (one probe against a template), on one core",
     "Durée d'une vérification (un contact contre un modèle)"),
    ("Error rates over random splits of enrolled / verification touches",
     "Taux d'erreur sur des découpages aléatoires entre contacts enregistrés et de vérification"),
    ("Also use partial touches (the driver would ask to repeat them)", "Utilise aussi les contacts partiels (le pilote les ferait répéter)"),
    ("{finger}: {n} partial touch(es) left out", "{finger} : {n} contact(s) partiel(s) exclu(s)"),
    ("genuine attempts accepted at the strictest threshold that rejects every impostor",
     "essais du bon doigt acceptés au seuil le plus strict qui rejette tous les mauvais doigts"),
    ("fixed threshold {t}: genuine accepted {g}%, impostors accepted {i}% (highest impostor score {w})",
     "seuil fixe {t} : bon doigt accepté {g} %, mauvais doigts acceptés {i} % (score le plus haut d'un mauvais doigt {w})"),
    ("Usage:", "Utilisation :"),
    ("Commands:", "Commandes :"),
    ("Options:", "Options :"),
    ("Arguments:", "Arguments :"),
    ("Print help", "Affiche l'aide"),
    ("Print version", "Affiche la version"),
];

const DE: &[(&str, &str)] = &[
    ("the sensor did not answer in time", "der Sensor hat nicht rechtzeitig geantwortet"),
    ("unexpected reply from the sensor: {msg}", "unerwartete Antwort des Sensors: {msg}"),
    ("the image is darker than the background: was the finger on the sensor during calibration?",
     "das Bild ist dunkler als der Hintergrund: lag der Finger während der Kalibrierung auf dem Sensor?"),
    ("calibration failed", "Kalibrierung fehlgeschlagen"),
    ("cancelled", "abgebrochen"),
    ("USB error: {msg}", "USB-Fehler: {msg}"),
    ("no 04f3:0c57 reader found, or no permission to open it (install udev/71-elan-0c57-uaccess.rules)",
     "kein Leser 04f3:0c57 gefunden oder keine Berechtigung (udev/71-elan-0c57-uaccess.rules installieren)"),
    ("the reader is in use (by fprintd?)", "der Leser ist belegt (durch fprintd?)"),
    ("Open source tool for the ELAN 04f3:0c57 fingerprint reader", "Freies Werkzeug für den Fingerabdruckleser ELAN 04f3:0c57"),
    ("Show firmware, sensor size and calibration values", "Zeigt Firmware, Sensorgröße und Kalibrierwerte"),
    ("Switch the LED on for a moment, to check that commands reach the reader", "Schaltet die LED kurz ein, um zu prüfen, dass Befehle den Leser erreichen"),
    ("How long to keep it on, in seconds", "Leuchtdauer in Sekunden"),
    ("Capture touches of one finger into a dataset folder", "Nimmt Berührungen eines Fingers in einen Datensatz-Ordner auf"),
    ("Finger label, used as sub-folder (e.g. right-index)", "Name des Fingers, als Unterordner verwendet (z. B. right-index)"),
    ("Number of touches to capture", "Anzahl der Berührungen"),
    ("Dataset folder", "Datensatz-Ordner"),
    ("Seconds to wait for each touch", "Wartezeit pro Berührung in Sekunden"),
    ("Also save touches that cover only part of the sensor", "Auch Berührungen speichern, die nur einen Teil des Sensors bedecken"),
    ("firmware:", "Firmware:"),
    ("sensor size:", "Sensorgröße:"),
    ("calibration mean:", "Kalibrier-Mittelwert:"),
    ("background mean:", "Hintergrund-Mittelwert:"),
    ("(after calibration)", "(nach der Kalibrierung)"),
    ("use letters, digits, '-' or '_' for the finger name", "für den Fingernamen Buchstaben, Ziffern, '-' oder '_' verwenden"),
    ("touch the sensor with {finger}…", "Sensor mit {finger} berühren…"),
    ("partial touch ({pct}% of the sensor): press the whole fingertip, try again",
     "unvollständige Berührung ({pct} % des Sensors): ganze Fingerkuppe auflegen und erneut versuchen"),
    ("saved {path}", "gespeichert: {path}"),
    ("no finger, stopping", "kein Finger, Abbruch"),
    ("{error} — lift the finger and try again", "{error} — Finger anheben und erneut versuchen"),
    ("lift the finger", "Finger anheben"),
    ("{n} touch(es) saved in {path}", "{n} Berührung(en) in {path} gespeichert"),
    ("Evaluate the fingerprint matcher on a dataset", "Bewertet den Fingerabdruck-Vergleich an einem Datensatz"),
    ("Print the scores of split 0 (first touches enrolled) as CSV, in the format of research/reference.py",
     "Gibt die Werte der Aufteilung 0 (erste Berührungen registriert) als CSV aus, im Format von research/reference.py"),
    ("Time of one verification (one probe against a template), on one core",
     "Dauer einer Prüfung (eine Berührung gegen eine Vorlage)"),
    ("Error rates over random splits of enrolled / verification touches",
     "Fehlerraten über zufällige Aufteilungen in registrierte und Prüf-Berührungen"),
    ("Also use partial touches (the driver would ask to repeat them)", "Auch unvollständige Berührungen verwenden (der Treiber ließe sie wiederholen)"),
    ("{finger}: {n} partial touch(es) left out", "{finger}: {n} unvollständige Berührung(en) ausgelassen"),
    ("genuine attempts accepted at the strictest threshold that rejects every impostor",
     "akzeptierte Versuche des richtigen Fingers bei der strengsten Schwelle, die jeden falschen Finger ablehnt"),
    ("fixed threshold {t}: genuine accepted {g}%, impostors accepted {i}% (highest impostor score {w})",
     "feste Schwelle {t}: richtiger Finger akzeptiert {g} %, falsche Finger akzeptiert {i} % (höchster Wert eines falschen Fingers {w})"),
    ("Usage:", "Aufruf:"),
    ("Commands:", "Befehle:"),
    ("Options:", "Optionen:"),
    ("Arguments:", "Argumente:"),
    ("Print help", "Hilfe anzeigen"),
    ("Print version", "Version anzeigen"),
];

const ES: &[(&str, &str)] = &[
    ("the sensor did not answer in time", "el sensor no respondió a tiempo"),
    ("unexpected reply from the sensor: {msg}", "respuesta inesperada del sensor: {msg}"),
    ("the image is darker than the background: was the finger on the sensor during calibration?",
     "la imagen es más oscura que el fondo: ¿estaba el dedo en el sensor durante la calibración?"),
    ("calibration failed", "falló la calibración"),
    ("cancelled", "cancelado"),
    ("USB error: {msg}", "error USB: {msg}"),
    ("no 04f3:0c57 reader found, or no permission to open it (install udev/71-elan-0c57-uaccess.rules)",
     "no se encontró ningún lector 04f3:0c57, o no hay permiso para abrirlo (instale udev/71-elan-0c57-uaccess.rules)"),
    ("the reader is in use (by fprintd?)", "el lector está en uso (¿por fprintd?)"),
    ("Open source tool for the ELAN 04f3:0c57 fingerprint reader", "Herramienta libre para el lector de huellas ELAN 04f3:0c57"),
    ("Show firmware, sensor size and calibration values", "Muestra el firmware, el tamaño del sensor y la calibración"),
    ("Switch the LED on for a moment, to check that commands reach the reader", "Enciende el LED un momento, para comprobar que las órdenes llegan al lector"),
    ("How long to keep it on, in seconds", "Cuánto tiempo encendido, en segundos"),
    ("Capture touches of one finger into a dataset folder", "Captura los toques de un dedo en una carpeta de datos"),
    ("Finger label, used as sub-folder (e.g. right-index)", "Nombre del dedo, usado como subcarpeta (p. ej. right-index)"),
    ("Number of touches to capture", "Número de toques a capturar"),
    ("Dataset folder", "Carpeta de datos"),
    ("Seconds to wait for each touch", "Segundos de espera por toque"),
    ("Also save touches that cover only part of the sensor", "Guarda también los toques que cubren solo parte del sensor"),
    ("firmware:", "firmware:"),
    ("sensor size:", "tamaño del sensor:"),
    ("calibration mean:", "media de calibración:"),
    ("background mean:", "media del fondo:"),
    ("(after calibration)", "(tras la calibración)"),
    ("use letters, digits, '-' or '_' for the finger name", "usa letras, cifras, '-' o '_' para el nombre del dedo"),
    ("touch the sensor with {finger}…", "toca el sensor con {finger}…"),
    ("partial touch ({pct}% of the sensor): press the whole fingertip, try again",
     "toque parcial ({pct} % del sensor): apoya toda la yema del dedo e inténtalo de nuevo"),
    ("saved {path}", "guardado {path}"),
    ("no finger, stopping", "ningún dedo, se detiene"),
    ("{error} — lift the finger and try again", "{error} — levanta el dedo e inténtalo de nuevo"),
    ("lift the finger", "levanta el dedo"),
    ("{n} touch(es) saved in {path}", "{n} toque(s) guardado(s) en {path}"),
    ("Evaluate the fingerprint matcher on a dataset", "Evalúa el algoritmo de comparación de huellas sobre un conjunto de datos"),
    ("Print the scores of split 0 (first touches enrolled) as CSV, in the format of research/reference.py",
     "Muestra las puntuaciones de la división 0 (primeros toques registrados) en CSV, con el formato de research/reference.py"),
    ("Time of one verification (one probe against a template), on one core",
     "Tiempo de una verificación (un toque contra un modelo)"),
    ("Error rates over random splits of enrolled / verification touches",
     "Tasas de error sobre divisiones aleatorias entre toques registrados y de verificación"),
    ("Also use partial touches (the driver would ask to repeat them)", "Usa también los toques parciales (el controlador los haría repetir)"),
    ("{finger}: {n} partial touch(es) left out", "{finger}: {n} toque(s) parcial(es) excluido(s)"),
    ("genuine attempts accepted at the strictest threshold that rejects every impostor",
     "intentos del dedo correcto aceptados con el umbral más estricto que rechaza todo dedo incorrecto"),
    ("fixed threshold {t}: genuine accepted {g}%, impostors accepted {i}% (highest impostor score {w})",
     "umbral fijo {t}: dedo correcto aceptado {g} %, dedos incorrectos aceptados {i} % (puntuación más alta de un dedo incorrecto {w})"),
    ("Usage:", "Uso:"),
    ("Commands:", "Órdenes:"),
    ("Options:", "Opciones:"),
    ("Arguments:", "Argumentos:"),
    ("Print help", "Muestra la ayuda"),
    ("Print version", "Muestra la versión"),
];

const PT: &[(&str, &str)] = &[
    ("the sensor did not answer in time", "o sensor não respondeu a tempo"),
    ("unexpected reply from the sensor: {msg}", "resposta inesperada do sensor: {msg}"),
    ("the image is darker than the background: was the finger on the sensor during calibration?",
     "a imagem é mais escura que o fundo: o dedo estava no sensor durante a calibração?"),
    ("calibration failed", "a calibração falhou"),
    ("cancelled", "cancelado"),
    ("USB error: {msg}", "erro USB: {msg}"),
    ("no 04f3:0c57 reader found, or no permission to open it (install udev/71-elan-0c57-uaccess.rules)",
     "nenhum leitor 04f3:0c57 encontrado, ou sem permissão para o abrir (instale udev/71-elan-0c57-uaccess.rules)"),
    ("the reader is in use (by fprintd?)", "o leitor está em uso (pelo fprintd?)"),
    ("Open source tool for the ELAN 04f3:0c57 fingerprint reader", "Ferramenta livre para o leitor de impressões digitais ELAN 04f3:0c57"),
    ("Show firmware, sensor size and calibration values", "Mostra o firmware, o tamanho do sensor e a calibração"),
    ("Switch the LED on for a moment, to check that commands reach the reader", "Acende o LED por um momento, para verificar que os comandos chegam ao leitor"),
    ("How long to keep it on, in seconds", "Quanto tempo aceso, em segundos"),
    ("Capture touches of one finger into a dataset folder", "Captura os toques de um dedo numa pasta de dados"),
    ("Finger label, used as sub-folder (e.g. right-index)", "Nome do dedo, usado como subpasta (ex. right-index)"),
    ("Number of touches to capture", "Número de toques a capturar"),
    ("Dataset folder", "Pasta de dados"),
    ("Seconds to wait for each touch", "Segundos de espera por toque"),
    ("Also save touches that cover only part of the sensor", "Guarda também os toques que cobrem só parte do sensor"),
    ("firmware:", "firmware:"),
    ("sensor size:", "tamanho do sensor:"),
    ("calibration mean:", "média de calibração:"),
    ("background mean:", "média do fundo:"),
    ("(after calibration)", "(após a calibração)"),
    ("use letters, digits, '-' or '_' for the finger name", "use letras, algarismos, '-' ou '_' para o nome do dedo"),
    ("touch the sensor with {finger}…", "toque no sensor com {finger}…"),
    ("partial touch ({pct}% of the sensor): press the whole fingertip, try again",
     "toque parcial ({pct}% do sensor): apoie toda a ponta do dedo e tente de novo"),
    ("saved {path}", "guardado {path}"),
    ("no finger, stopping", "nenhum dedo, a parar"),
    ("{error} — lift the finger and try again", "{error} — levante o dedo e tente de novo"),
    ("lift the finger", "levante o dedo"),
    ("{n} touch(es) saved in {path}", "{n} toque(s) guardado(s) em {path}"),
    ("Evaluate the fingerprint matcher on a dataset", "Avalia o algoritmo de comparação de impressões num conjunto de dados"),
    ("Print the scores of split 0 (first touches enrolled) as CSV, in the format of research/reference.py",
     "Mostra as pontuações da divisão 0 (primeiros toques registados) em CSV, no formato de research/reference.py"),
    ("Time of one verification (one probe against a template), on one core",
     "Tempo de uma verificação (um toque contra um modelo)"),
    ("Error rates over random splits of enrolled / verification touches",
     "Taxas de erro em divisões aleatórias entre toques registados e de verificação"),
    ("Also use partial touches (the driver would ask to repeat them)", "Usa também os toques parciais (o controlador pedi-los-ia de novo)"),
    ("{finger}: {n} partial touch(es) left out", "{finger}: {n} toque(s) parcial(is) excluído(s)"),
    ("genuine attempts accepted at the strictest threshold that rejects every impostor",
     "tentativas do dedo certo aceites com o limiar mais estrito que rejeita todos os dedos errados"),
    ("fixed threshold {t}: genuine accepted {g}%, impostors accepted {i}% (highest impostor score {w})",
     "limiar fixo {t}: dedo certo aceite {g}%, dedos errados aceites {i}% (pontuação mais alta de um dedo errado {w})"),
    ("Usage:", "Utilização:"),
    ("Commands:", "Comandos:"),
    ("Options:", "Opções:"),
    ("Arguments:", "Argumentos:"),
    ("Print help", "Mostra a ajuda"),
    ("Print version", "Mostra a versão"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(s: &str) -> Vec<&str> {
        let mut v: Vec<&str> = s.split('{').skip(1).filter_map(|p| p.split('}').next()).collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn every_language_translates_every_message_with_the_same_fields() {
        let ids: Vec<&str> = IT.iter().map(|(id, _)| *id).collect();
        for (lang, table) in TABLES {
            assert_eq!(table.iter().map(|(id, _)| *id).collect::<Vec<_>>(), ids, "{lang}");
            for (id, text) in table {
                assert_eq!(fields(id), fields(text), "{lang}: {id}");
            }
        }
    }

    #[test]
    fn placeholders_are_filled() {
        assert_eq!(trf("saved {path}", &[("path", &"x.png")]).ends_with("x.png"), true);
    }
}
