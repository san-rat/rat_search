#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Rat Search"
APP_COMMAND="rat-search"
HOTKEY_COMMAND="/bin/sh -c 'exec rat-search foreground --startup-id \"\$DESKTOP_STARTUP_ID\" --xdg-activation-token \"\$XDG_ACTIVATION_TOKEN\"'"
HOTKEY_BINDING="<Control><Alt>space"
KEY_SCHEMA="org.gnome.settings-daemon.plugins.media-keys"
KEY_BASE="/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_TEMPLATE="${REPO_ROOT}/packaging/linux/rat-search.desktop"
AUTOSTART_TEMPLATE="${REPO_ROOT}/packaging/linux/rat-search-autostart.desktop"
ICON_SOURCE="${REPO_ROOT}/src-tauri/icons/128x128.png"
APP_ENTRY="${HOME}/.local/share/applications/rat-search.desktop"
SYSTEM_APP_ENTRY="/usr/share/applications/Rat Search.desktop"
AUTOSTART_ENTRY="${HOME}/.config/autostart/rat-search.desktop"
ICON_TARGET="${HOME}/.local/share/icons/hicolor/128x128/apps/rat-search.png"
LOG_DIR="${HOME}/.local/state/rat-search"
LOG_FILE="${LOG_DIR}/setup.log"

log() {
  mkdir -p "${LOG_DIR}"
  printf '%s setup: %s\n' "$(date '+%Y-%m-%d %H:%M:%S %z')" "$*" | tee -a "${LOG_FILE}"
}

fail() {
  log "ERROR: $*"
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "Required command '$1' was not found."
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
  gsettings set "${KEY_SCHEMA}.custom-keybinding:${path}" "${key}" "${value}"
}

gvariant_string() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '"%s"' "${value}"
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

find_free_path() {
  local existing="$1"
  local index
  local candidate
  for index in $(seq 0 99); do
    candidate="${KEY_BASE}/custom${index}/"
    if ! grep -Fxq "${candidate}" <<<"${existing}"; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  return 1
}

configure_hotkey() {
  require_command gsettings

  local existing_paths
  local target_path=""
  local path
  local binding
  local next_paths=()

  existing_paths="$(get_custom_paths)"

  while IFS= read -r path; do
    [[ -n "${path}" ]] || continue
    binding="$(key_value "${path}" binding)"

    if is_rat_search_binding "${path}"; then
      if [[ -z "${target_path}" ]]; then
        target_path="${path}"
        next_paths+=("${path}")
      else
        set_key_value "${path}" binding "''"
      fi
      continue
    fi

    if [[ "${binding}" == "${HOTKEY_BINDING}" ]]; then
      fail "Shortcut ${HOTKEY_BINDING} is already used by another custom keybinding at ${path}."
    fi

    next_paths+=("${path}")
  done <<<"${existing_paths}"

  if [[ -z "${target_path}" ]]; then
    target_path="$(find_free_path "${existing_paths}")" ||
      fail "Could not find a free GNOME custom keybinding slot."
    next_paths+=("${target_path}")
  fi

  gsettings set "${KEY_SCHEMA}" custom-keybindings "$(format_path_array "${next_paths[@]}")"
  set_key_value "${target_path}" name "$(gvariant_string "${APP_NAME}")"
  set_key_value "${target_path}" command "$(gvariant_string "${HOTKEY_COMMAND}")"
  set_key_value "${target_path}" binding "$(gvariant_string "${HOTKEY_BINDING}")"

  log "Configured GNOME shortcut ${HOTKEY_BINDING} -> ${HOTKEY_COMMAND}."
}

main() {
  log "Starting ${APP_NAME} current-user setup."

  if [[ "${XDG_CURRENT_DESKTOP:-}" != *GNOME* ]]; then
    log "WARNING: XDG_CURRENT_DESKTOP='${XDG_CURRENT_DESKTOP:-<unset>}' does not look like GNOME."
  fi

  if [[ "${XDG_SESSION_TYPE:-}" != "wayland" ]]; then
    log "WARNING: XDG_SESSION_TYPE='${XDG_SESSION_TYPE:-<unset>}' is not wayland."
  fi

  require_command "${APP_COMMAND}"
  [[ -f "${APP_TEMPLATE}" ]] || fail "Missing template ${APP_TEMPLATE}."
  [[ -f "${AUTOSTART_TEMPLATE}" ]] || fail "Missing template ${AUTOSTART_TEMPLATE}."

  if [[ -f "${SYSTEM_APP_ENTRY}" ]]; then
    rm -f "${APP_ENTRY}"
    log "Using package desktop entry ${SYSTEM_APP_ENTRY}; removed duplicate user entry if present."
  else
    install -D -m 0644 "${APP_TEMPLATE}" "${APP_ENTRY}"
    log "Installed desktop entry ${APP_ENTRY}."
  fi

  install -D -m 0644 "${AUTOSTART_TEMPLATE}" "${AUTOSTART_ENTRY}"
  log "Installed autostart entry ${AUTOSTART_ENTRY}."

  if [[ -f "${ICON_SOURCE}" ]]; then
    install -D -m 0644 "${ICON_SOURCE}" "${ICON_TARGET}"
    log "Installed icon ${ICON_TARGET}."
  else
    log "WARNING: Icon source ${ICON_SOURCE} was not found."
  fi

  configure_hotkey
  log "${APP_NAME} setup complete."
}

main "$@"
