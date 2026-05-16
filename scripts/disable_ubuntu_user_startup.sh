#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Rat Search"
HOTKEY_COMMAND="/bin/sh -c 'exec rat-search foreground --startup-id \"\$DESKTOP_STARTUP_ID\" --xdg-activation-token \"\$XDG_ACTIVATION_TOKEN\"'"
KEY_SCHEMA="org.gnome.settings-daemon.plugins.media-keys"
AUTOSTART_ENTRY="${HOME}/.config/autostart/rat-search.desktop"
LOG_DIR="${HOME}/.local/state/rat-search"
LOG_FILE="${LOG_DIR}/setup.log"

log() {
  mkdir -p "${LOG_DIR}"
  printf '%s disable: %s\n' "$(date '+%Y-%m-%d %H:%M:%S %z')" "$*" | tee -a "${LOG_FILE}"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    log "ERROR: Required command '$1' was not found."
    exit 1
  }
}

get_custom_paths() {
  gsettings get "${KEY_SCHEMA}" custom-keybindings |
    tr -d "[]'," |
    tr ' ' '\n' |
    sed '/^$/d; /^@as$/d'
}

format_path_array() {
  local first=1
  printf '['
  for path in "$@"; do
    if [[ "${first}" -eq 0 ]]; then
      printf ', '
    fi
    printf "'%s'" "${path}"
    first=0
  done
  printf ']'
}

key_value() {
  local path="$1"
  local key="$2"
  gsettings get "${KEY_SCHEMA}.custom-keybinding:${path}" "${key}" 2>/dev/null |
    sed "s/^'//; s/'$//"
}

set_key_value() {
  local path="$1"
  local key="$2"
  local value="$3"
  gsettings set "${KEY_SCHEMA}.custom-keybinding:${path}" "${key}" "${value}" 2>/dev/null || true
}

is_rat_search_binding() {
  local path="$1"
  local name
  local command
  name="$(key_value "${path}" name)"
  command="$(key_value "${path}" command)"
  [[ "${name}" == "${APP_NAME}" ||
    "${name}" == "rat-search toggle" ||
    "${command}" == "${HOTKEY_COMMAND}" ||
    "${command}" == *"rat-search toggle"* ||
    "${command}" == *"rat-search foreground"* ]]
}

remove_hotkey() {
  require_command gsettings

  local kept_paths=()
  local removed_count=0
  local path

  while IFS= read -r path; do
    [[ -n "${path}" ]] || continue

    if is_rat_search_binding "${path}"; then
      set_key_value "${path}" binding "''"
      set_key_value "${path}" command "''"
      set_key_value "${path}" name "''"
      removed_count=$((removed_count + 1))
      continue
    fi

    kept_paths+=("${path}")
  done <<<"$(get_custom_paths)"

  gsettings set "${KEY_SCHEMA}" custom-keybindings "$(format_path_array "${kept_paths[@]}")"
  log "Removed ${removed_count} Rat Search GNOME custom keybinding(s)."
}

main() {
  log "Starting ${APP_NAME} startup disable."

  if [[ -f "${AUTOSTART_ENTRY}" ]]; then
    rm -f "${AUTOSTART_ENTRY}"
    log "Removed autostart entry ${AUTOSTART_ENTRY}."
  else
    log "Autostart entry ${AUTOSTART_ENTRY} was already absent."
  fi

  remove_hotkey
  log "${APP_NAME} startup disable complete."
}

main "$@"
