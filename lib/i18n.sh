# shellcheck shell=bash
# SPDX-License-Identifier: LGPL-2.1-or-later
#
# Messages of build.sh, install.sh and uninstall.sh in the language of the
# system: English, Italian, French, German, Spanish or Portuguese (English
# for any other language).
#
#   msg KEY [ARGS...]     prints message KEY, printf-style
#   die KEY [ARGS...]     prints it as an error and exits
#   step KEY [ARGS...]    prints it as the title of a step

# The session language (kept by sudo), else the system one.
_locale=""
for _v in "${LC_ALL:-}" "${LC_MESSAGES:-}" "${LANG:-}"; do
  case "$_v" in
    "" | C | POSIX | C.*) ;;
    *) _locale="$_v"; break ;;
  esac
done
if [ -n "$_locale" ] && [ -n "${LANGUAGE:-}" ]; then
  _locale="${LANGUAGE%%:*}"
fi
if [ -z "$_locale" ] && [ -r /etc/locale.conf ]; then
  _locale="$(sed -n 's/^LANG="\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' /etc/locale.conf | head -n 1)"
fi
I18N_LANG="${_locale%%[_.@-]*}"
case "$I18N_LANG" in
  it | fr | de | es | pt) ;;
  *) I18N_LANG=en ;;
esac
unset _locale _v

msg () {
  local key="$1" text fallback
  shift
  text="M_${I18N_LANG}_${key}"
  fallback="M_en_${key}"
  # shellcheck disable=SC2059
  printf "${!text:-${!fallback}}\n" "$@"
}

die () {
  local prefix="M_${I18N_LANG}_error"
  printf '%s ' "${!prefix}" >&2
  msg "$@" >&2
  exit 1
}

step () {
  echo
  printf '==> '
  msg "$@"
}

# -- English ------------------------------------------------------------------
M_en_error="ERROR:"
M_en_run_as_user="run %s as a normal user, not as root"
M_en_run_as_root="run as root: sudo %s"
M_en_missing_tool="missing tool: %s (see README.md)"
M_en_missing_pkg="missing development package for '%s' (see README.md)"
M_en_fetch_tod="Fetching libfprint-TOD (%s)"
M_en_build_tod="Building libfprint-TOD (prefix %s, libdir %s)"
M_en_build_driver="Building the open source driver (Rust)"
M_en_build_done="Done. Files ready to be installed:"
M_en_next_install="Now run:  sudo ./install.sh"
M_en_not_built="nothing to install, run ./build.sh first (as a normal user)"
M_en_incomplete="incomplete build, run ./build.sh again"
M_en_backup="Backing up the existing libfprint in %s to %s"
M_en_backup_failed="backup check failed for %s"
M_en_backup_ok="backup verified"
M_en_installing="Installing into %s"
M_en_ldconf="Adding %s to the dynamic linker path (%s)"
M_en_legacy="Removing ELAN's proprietary driver, no longer needed"
M_en_removed="removed %s"
M_en_check="Check"
M_en_fprintd_other_lib="WARNING: fprintd does not use %s"
M_en_no_fprintd="WARNING: fprintd not found, install it with your package manager"
M_en_installed="Installed. Now enroll a finger as your user (12 touches, move the finger a bit each time):"
M_en_then_test="and test it with:"
M_en_old_prints="Fingerprints enrolled with ELAN's driver can't be used: enroll them again."
M_en_password="Your password always works for login and sudo."
M_en_no_install="no installation found in %s"
M_en_restoring="Restoring the previous libfprint from %s"
M_en_uninstalled="Uninstalled. Prints enrolled with this driver can't be used by other drivers, remove them with:"

# -- Italiano -----------------------------------------------------------------
M_it_error="ERRORE:"
M_it_run_as_user="esegui %s come utente normale, non come root"
M_it_run_as_root="esegui come root: sudo %s"
M_it_missing_tool="strumento mancante: %s (vedi README.md)"
M_it_missing_pkg="manca il pacchetto di sviluppo per '%s' (vedi README.md)"
M_it_fetch_tod="Scaricamento di libfprint-TOD (%s)"
M_it_build_tod="Compilazione di libfprint-TOD (prefisso %s, libdir %s)"
M_it_build_driver="Compilazione del driver open source (Rust)"
M_it_build_done="Fatto. File pronti per l'installazione:"
M_it_next_install="Ora esegui:  sudo ./install.sh"
M_it_not_built="niente da installare, esegui prima ./build.sh (come utente normale)"
M_it_incomplete="compilazione incompleta, esegui di nuovo ./build.sh"
M_it_backup="Copia di sicurezza della libfprint presente in %s in %s"
M_it_backup_failed="verifica della copia di sicurezza fallita per %s"
M_it_backup_ok="copia di sicurezza verificata"
M_it_installing="Installazione in %s"
M_it_ldconf="Aggiunta di %s ai percorsi del linker dinamico (%s)"
M_it_legacy="Rimozione del driver proprietario ELAN, non più necessario"
M_it_removed="rimosso %s"
M_it_check="Controllo"
M_it_fprintd_other_lib="ATTENZIONE: fprintd non usa %s"
M_it_no_fprintd="ATTENZIONE: fprintd non trovato, installalo con il gestore dei pacchetti"
M_it_installed="Installato. Ora registra un dito come utente normale (12 tocchi, spostando un po' il dito ogni volta):"
M_it_then_test="e provalo con:"
M_it_old_prints="Le impronte registrate con il driver ELAN non si possono usare: registrale di nuovo."
M_it_password="La password funziona sempre per l'accesso e per sudo."
M_it_no_install="nessuna installazione trovata in %s"
M_it_restoring="Ripristino della libfprint precedente da %s"
M_it_uninstalled="Disinstallato. Le impronte registrate con questo driver non valgono per altri driver, eliminale con:"

# -- Français -----------------------------------------------------------------
M_fr_error="ERREUR :"
M_fr_run_as_user="lancez %s en tant qu'utilisateur normal, pas en root"
M_fr_run_as_root="lancez en root : sudo %s"
M_fr_missing_tool="outil manquant : %s (voir README.md)"
M_fr_missing_pkg="paquet de développement manquant pour '%s' (voir README.md)"
M_fr_fetch_tod="Téléchargement de libfprint-TOD (%s)"
M_fr_build_tod="Compilation de libfprint-TOD (préfixe %s, libdir %s)"
M_fr_build_driver="Compilation du pilote libre (Rust)"
M_fr_build_done="Terminé. Fichiers prêts à être installés :"
M_fr_next_install="Lancez maintenant :  sudo ./install.sh"
M_fr_not_built="rien à installer, lancez d'abord ./build.sh (en utilisateur normal)"
M_fr_incomplete="compilation incomplète, relancez ./build.sh"
M_fr_backup="Sauvegarde de la libfprint présente dans %s vers %s"
M_fr_backup_failed="échec de la vérification de la sauvegarde pour %s"
M_fr_backup_ok="sauvegarde vérifiée"
M_fr_installing="Installation dans %s"
M_fr_ldconf="Ajout de %s aux chemins de l'éditeur de liens (%s)"
M_fr_legacy="Suppression du pilote propriétaire ELAN, devenu inutile"
M_fr_removed="supprimé %s"
M_fr_check="Vérification"
M_fr_fprintd_other_lib="ATTENTION : fprintd n'utilise pas %s"
M_fr_no_fprintd="ATTENTION : fprintd introuvable, installez-le avec votre gestionnaire de paquets"
M_fr_installed="Installé. Enregistrez maintenant un doigt avec votre utilisateur (12 contacts, en déplaçant un peu le doigt à chaque fois) :"
M_fr_then_test="puis testez-le avec :"
M_fr_old_prints="Les empreintes enregistrées avec le pilote ELAN ne sont pas utilisables : enregistrez-les à nouveau."
M_fr_password="Votre mot de passe fonctionne toujours pour la connexion et sudo."
M_fr_no_install="aucune installation trouvée dans %s"
M_fr_restoring="Restauration de la libfprint précédente depuis %s"
M_fr_uninstalled="Désinstallé. Les empreintes enregistrées avec ce pilote ne servent pas aux autres pilotes, supprimez-les avec :"

# -- Deutsch ------------------------------------------------------------------
M_de_error="FEHLER:"
M_de_run_as_user="%s als normaler Benutzer ausführen, nicht als root"
M_de_run_as_root="als root ausführen: sudo %s"
M_de_missing_tool="fehlendes Werkzeug: %s (siehe README.md)"
M_de_missing_pkg="fehlendes Entwicklungspaket für '%s' (siehe README.md)"
M_de_fetch_tod="libfprint-TOD wird heruntergeladen (%s)"
M_de_build_tod="libfprint-TOD wird gebaut (Präfix %s, libdir %s)"
M_de_build_driver="Der freie Treiber (Rust) wird gebaut"
M_de_build_done="Fertig. Zu installierende Dateien:"
M_de_next_install="Jetzt ausführen:  sudo ./install.sh"
M_de_not_built="nichts zu installieren, zuerst ./build.sh ausführen (als normaler Benutzer)"
M_de_incomplete="unvollständiger Build, ./build.sh erneut ausführen"
M_de_backup="Sicherung der vorhandenen libfprint aus %s nach %s"
M_de_backup_failed="Prüfung der Sicherung fehlgeschlagen für %s"
M_de_backup_ok="Sicherung geprüft"
M_de_installing="Installation nach %s"
M_de_ldconf="%s wird zum Pfad des dynamischen Linkers hinzugefügt (%s)"
M_de_legacy="Der proprietäre ELAN-Treiber wird entfernt, er wird nicht mehr benötigt"
M_de_removed="entfernt: %s"
M_de_check="Prüfung"
M_de_fprintd_other_lib="WARNUNG: fprintd verwendet %s nicht"
M_de_no_fprintd="WARNUNG: fprintd nicht gefunden, mit der Paketverwaltung installieren"
M_de_installed="Installiert. Jetzt als normaler Benutzer einen Finger registrieren (12 Berührungen, den Finger jedes Mal etwas verschieben):"
M_de_then_test="und testen mit:"
M_de_old_prints="Mit dem ELAN-Treiber registrierte Fingerabdrücke sind nicht verwendbar: bitte neu registrieren."
M_de_password="Das Passwort funktioniert für Anmeldung und sudo weiterhin."
M_de_no_install="keine Installation in %s gefunden"
M_de_restoring="Die vorherige libfprint wird aus %s wiederhergestellt"
M_de_uninstalled="Deinstalliert. Mit diesem Treiber registrierte Fingerabdrücke gelten nicht für andere Treiber, entfernen mit:"

# -- Español ------------------------------------------------------------------
M_es_error="ERROR:"
M_es_run_as_user="ejecute %s como usuario normal, no como root"
M_es_run_as_root="ejecute como root: sudo %s"
M_es_missing_tool="falta la herramienta: %s (vea README.md)"
M_es_missing_pkg="falta el paquete de desarrollo de '%s' (vea README.md)"
M_es_fetch_tod="Descargando libfprint-TOD (%s)"
M_es_build_tod="Compilando libfprint-TOD (prefijo %s, libdir %s)"
M_es_build_driver="Compilando el controlador libre (Rust)"
M_es_build_done="Hecho. Archivos listos para instalar:"
M_es_next_install="Ahora ejecute:  sudo ./install.sh"
M_es_not_built="nada que instalar, ejecute primero ./build.sh (como usuario normal)"
M_es_incomplete="compilación incompleta, ejecute ./build.sh de nuevo"
M_es_backup="Copia de seguridad de la libfprint existente en %s en %s"
M_es_backup_failed="falló la verificación de la copia de seguridad de %s"
M_es_backup_ok="copia de seguridad verificada"
M_es_installing="Instalando en %s"
M_es_ldconf="Añadiendo %s a las rutas del enlazador dinámico (%s)"
M_es_legacy="Eliminando el controlador propietario de ELAN, ya no es necesario"
M_es_removed="eliminado %s"
M_es_check="Comprobación"
M_es_fprintd_other_lib="AVISO: fprintd no usa %s"
M_es_no_fprintd="AVISO: no se encontró fprintd, instálelo con el gestor de paquetes"
M_es_installed="Instalado. Ahora registre un dedo con su usuario (12 toques, moviendo un poco el dedo cada vez):"
M_es_then_test="y pruébelo con:"
M_es_old_prints="Las huellas registradas con el controlador de ELAN no se pueden usar: regístrelas de nuevo."
M_es_password="La contraseña siempre funciona para iniciar sesión y para sudo."
M_es_no_install="no se encontró ninguna instalación en %s"
M_es_restoring="Restaurando la libfprint anterior desde %s"
M_es_uninstalled="Desinstalado. Las huellas registradas con este controlador no sirven para otros, elimínelas con:"

# -- Português ----------------------------------------------------------------
M_pt_error="ERRO:"
M_pt_run_as_user="execute %s como utilizador normal, não como root"
M_pt_run_as_root="execute como root: sudo %s"
M_pt_missing_tool="ferramenta em falta: %s (veja README.md)"
M_pt_missing_pkg="falta o pacote de desenvolvimento de '%s' (veja README.md)"
M_pt_fetch_tod="A transferir libfprint-TOD (%s)"
M_pt_build_tod="A compilar libfprint-TOD (prefixo %s, libdir %s)"
M_pt_build_driver="A compilar o controlador livre (Rust)"
M_pt_build_done="Concluído. Ficheiros prontos para instalar:"
M_pt_next_install="Agora execute:  sudo ./install.sh"
M_pt_not_built="nada para instalar, execute primeiro ./build.sh (como utilizador normal)"
M_pt_incomplete="compilação incompleta, execute ./build.sh novamente"
M_pt_backup="Cópia de segurança da libfprint existente em %s para %s"
M_pt_backup_failed="falhou a verificação da cópia de segurança de %s"
M_pt_backup_ok="cópia de segurança verificada"
M_pt_installing="A instalar em %s"
M_pt_ldconf="A adicionar %s aos caminhos do linker dinâmico (%s)"
M_pt_legacy="A remover o controlador proprietário da ELAN, já não é necessário"
M_pt_removed="removido %s"
M_pt_check="Verificação"
M_pt_fprintd_other_lib="AVISO: o fprintd não usa %s"
M_pt_no_fprintd="AVISO: fprintd não encontrado, instale-o com o gestor de pacotes"
M_pt_installed="Instalado. Agora registe um dedo com o seu utilizador (12 toques, mexendo um pouco o dedo de cada vez):"
M_pt_then_test="e teste-o com:"
M_pt_old_prints="As impressões registadas com o controlador da ELAN não podem ser usadas: registe-as de novo."
M_pt_password="A palavra-passe funciona sempre para iniciar sessão e para sudo."
M_pt_no_install="nenhuma instalação encontrada em %s"
M_pt_restoring="A restaurar a libfprint anterior a partir de %s"
M_pt_uninstalled="Desinstalado. As impressões registadas com este controlador não servem para outros, remova-as com:"
