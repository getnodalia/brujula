#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Fase 0 del DRM con CEF (docs/drm-cef.md): ¿carga Widevine una app CEF con el CDM de Chrome?
# SIN PROBAR: escrito desde la nube, que no alcanza cef-builds.spotifycdn.com. Ejecutar en el PC
# de Linux de Daniel (CachyOS) con Chrome instalado:  bash drm-fase0.sh
# No instala nada del sistema; todo queda en ~/.cache/brujula-drm-fase0.
set -euo pipefail

work="${HOME}/.cache/brujula-drm-fase0"
mkdir -p "$work"
cd "$work"

for c in curl python3 tar cmake ninja g++; do
  command -v "$c" >/dev/null || { echo "Falta '$c' (en Arch: sudo pacman -S --needed base-devel cmake ninja python curl)"; exit 1; }
done

# 1. Buscar el CDM de Widevine de Chrome / Chromium / Edge / Brave
cdm=""
for d in /opt/google/chrome/WidevineCdm /opt/google/chrome-beta/WidevineCdm /opt/microsoft/msedge/WidevineCdm \
         "$HOME"/.config/google-chrome/WidevineCdm/* "$HOME"/.config/chromium/WidevineCdm/* \
         "$HOME"/.config/BraveSoftware/Brave-Browser/WidevineCdm/*; do
  [ -f "$d/manifest.json" ] || continue
  [ -f "$d/_platform_specific/linux_x64/libwidevinecdm.so" ] || [ -f "$d/libwidevinecdm.so" ] || continue
  cdm="$d"
done
if [ -z "$cdm" ]; then
  # Búsqueda amplia (Flatpak, Chrome en otra ruta, Brave, Vivaldi…): sube desde la biblioteca hasta el manifest.json
  while IFS= read -r so; do
    d="$(dirname "$so")"
    while [ "$d" != "/" ] && [ ! -f "$d/manifest.json" ]; do d="$(dirname "$d")"; done
    if [ -f "$d/manifest.json" ]; then cdm="$d"; fi
  done < <(find "$HOME/.config" "$HOME/.var/app" "$HOME/.local/share" /opt /usr/lib /usr/lib64 -name libwidevinecdm.so 2>/dev/null)
fi
if [ -z "$cdm" ]; then echo "No encuentro un WidevineCdm. Abre Chrome una vez (lo descarga) y repite."; exit 1; fi
cdm_version="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' "$cdm/manifest.json")"
echo "CDM: $cdm (versión $cdm_version)"

# 2. Descargar la última CEF estable de Linux x64 (comprobando el sha1 del índice)
curl -fsSL https://cef-builds.spotifycdn.com/index.json -o index.json
read -r name sha1 < <(python3 - <<'PY'
import json
v = [x for x in json.load(open("index.json"))["linux64"]["versions"] if x.get("channel") == "stable"]
f = [f for f in v[0]["files"] if f["type"] == "standard"][0]
print(f["name"], f["sha1"])
PY
)
echo "CEF: $name"
if [ ! -f "$name" ]; then curl -fL "https://cef-builds.spotifycdn.com/$name" -o "$name"; fi
echo "$sha1  $name" | sha1sum -c -
dir="${name%.tar.bz2}"
[ -d "$dir" ] || tar xjf "$name"

# 3. Compilar cefsimple (la ventana de ejemplo de CEF)
cd "$dir"
mkdir -p build && cd build
cmake -G Ninja -DCMAKE_BUILD_TYPE=Release .. >/dev/null
ninja cefsimple
bin="$(find . -type f -name cefsimple -perm -u+x | head -1)"
[ -n "$bin" ] || { echo "No salió cefsimple"; exit 1; }

# 4. Probar. --no-sandbox solo para esta prueba.
url="${1:-https://integration.widevine.com/player}"
echo "Abriendo $url — comprueba si reproduce; luego prueba Netflix y Spotify cambiando la URL: bash $0 https://www.netflix.com"
cd "$(dirname "$bin")"
./cefsimple --no-sandbox --url="$url" \
  --widevine-cdm-path="$cdm" --widevine-cdm-version="$cdm_version" 2>&1 | tee "$work/cefsimple.log" | grep -iE 'widevine|cdm|drm|eme' || true
echo "Registro completo: $work/cefsimple.log"
