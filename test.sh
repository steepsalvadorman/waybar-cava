#!/bin/bash
# Tests de instalación y funcionamiento de ags-cavars.
# Ejecuta: bash test.sh
# No modifica ningún archivo del sistema.

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0

pass() { echo -e "  ${GREEN}✓${NC} $1"; ((PASS++)); }
fail() { echo -e "  ${RED}✗${NC} $1"; ((FAIL++)); }
skip() { echo -e "  ${YELLOW}–${NC} $1 ${YELLOW}(omitido)${NC}"; ((SKIP++)); }

# Genera N frames de CAVA (i16 LE) con amplitud dada (0–32767).
# Uso: cava_frames <num_frames> <amplitud>
cava_frames() {
    python3 -c "
import struct, sys
frames = $1
amp    = $2
channels = 16
data = struct.pack('<' + 'h' * channels, *[amp] * channels) * frames
sys.stdout.buffer.write(data)
"
}

BIN="$HOME/.local/bin/ags-cavars"

echo ""
echo -e "  ${BOLD}ags-cavars · tests${NC}"
echo "  ───────────────────────────────"

# ── Bloque 1: Entorno ────────────────────────────────────────────────────────

echo ""
echo -e "  ${BOLD}Entorno${NC}"

if command -v cargo &>/dev/null; then
    pass "Rust instalado ($(cargo --version 2>/dev/null | head -1))"
else
    fail "Rust no está instalado — ve a https://rustup.rs"
fi

if command -v cava &>/dev/null; then
    pass "CAVA instalado ($(cava --version 2>/dev/null | head -1 || echo 'versión desconocida'))"
else
    fail "CAVA no está instalado — instálalo con: sudo pacman -S cava"
fi

if command -v python3 &>/dev/null; then
    pass "Python3 disponible (necesario para generar datos de prueba)"
else
    fail "Python3 no disponible — algunos tests no pueden ejecutarse"
fi

if echo "$PATH" | grep -q "$HOME/.local/bin"; then
    pass "~/.local/bin está en PATH"
else
    fail "~/.local/bin NO está en PATH — añade: export PATH=\"\$HOME/.local/bin:\$PATH\" a tu ~/.zshrc"
fi

# ── Bloque 2: Compilación ────────────────────────────────────────────────────

echo ""
echo -e "  ${BOLD}Compilación${NC}"

if RUSTFLAGS="-A warnings" cargo build --release -q 2>/dev/null; then
    pass "cargo build --release funciona"
else
    fail "cargo build --release falló — revisa los errores con: cargo build --release"
fi

if [ -f "target/release/ags-cavars" ]; then
    pass "Binario generado en target/release/ags-cavars"
else
    fail "Binario NO encontrado en target/release/ags-cavars"
fi

if cargo test -q 2>/dev/null | grep -q "test result: ok"; then
    COUNT=$(cargo test -q 2>/dev/null | grep "test result" | grep -o '[0-9]* passed' | head -1)
    pass "Tests unitarios de Rust pasan ($COUNT)"
else
    fail "Algún test unitario de Rust falla — ejecuta: cargo test"
fi

# ── Bloque 3: Instalación ────────────────────────────────────────────────────

echo ""
echo -e "  ${BOLD}Instalación${NC}"

if [ -f "$BIN" ]; then
    pass "Binario instalado en $BIN"
else
    fail "Binario NO instalado en $BIN — ejecuta: ./install.sh"
fi

if [ -x "$BIN" ]; then
    pass "Binario es ejecutable"
else
    fail "Binario no es ejecutable — ejecuta: chmod +x $BIN"
fi

if [ -f "$HOME/.config/cava/ags.ini" ]; then
    pass "Config de CAVA en ~/.config/cava/ags.ini"
else
    fail "Config de CAVA NO encontrado en ~/.config/cava/ags.ini — ejecuta: ./install.sh"
fi

# ── Bloque 4: Config de CAVA ─────────────────────────────────────────────────

echo ""
echo -e "  ${BOLD}Config de CAVA${NC}"

INI="$HOME/.config/cava/ags.ini"

if [ -f "$INI" ]; then
    if grep -q "method = raw" "$INI"; then
        pass "Config tiene salida raw (necesario para el pipeline)"
    else
        fail "Config NO tiene 'method = raw' en [output] — el pipeline no funcionará"
    fi

    if grep -q "bit_format = 16bit" "$INI"; then
        pass "Config usa formato 16bit"
    else
        fail "Config NO tiene 'bit_format = 16bit' — el binario no podrá leer los datos"
    fi

    BARS=$(grep -E "^bars\s*=" "$INI" | grep -o '[0-9]*' | head -1)
    if [ "$BARS" = "16" ]; then
        pass "Config tiene 16 barras (coincide con CHANNELS en el código)"
    else
        fail "Config tiene $BARS barras pero el código espera 16 — ajusta 'bars' o cambia CHANNELS en src/main.rs"
    fi
else
    skip "Config no existe, tests de config omitidos"
fi

# ── Bloque 5: Comportamiento del binario ─────────────────────────────────────

echo ""
echo -e "  ${BOLD}Comportamiento del binario${NC}"

if ! [ -f "$BIN" ] || ! command -v python3 &>/dev/null; then
    skip "Binario o python3 no disponible, tests de comportamiento omitidos"
else
    # Señal activa — debe producir <span con color
    OUT=$(cava_frames 3 16383 | "$BIN" --ags --led 2>/dev/null | head -1)
    if echo "$OUT" | grep -q "<span"; then
        pass "Señal activa → produce Pango Markup (<span color=...>)"
    else
        fail "Señal activa NO produce Pango Markup — salida: $OUT"
    fi

    # Modo JSON (sin --ags) — debe producir objeto JSON
    OUT_JSON=$(cava_frames 1 16383 | "$BIN" --led 2>/dev/null | head -1)
    if echo "$OUT_JSON" | grep -q '"text"'; then
        pass "Sin --ags → produce JSON con campo 'text' (compatible con Waybar)"
    else
        fail "Sin --ags NO produce JSON — salida: $OUT_JSON"
    fi

    # Silencio sostenido — a los 45 frames el estado cambia a Silent (color #8a8ea8).
    # Usamos 60 frames; tras el frame 45 aparece el standby. El último frame es EOF
    # que emite error, así que buscamos en todas las líneas, no solo la última.
    OUT_SIL=$(cava_frames 60 0 | "$BIN" --ags --led 2>/dev/null)
    if echo "$OUT_SIL" | grep -q "#8a8ea8"; then
        pass "Silencio sostenido → produce markup de standby"
    else
        fail "Silencio sostenido NO produce markup de standby"
    fi

    # EOF inmediato — debe producir markup de error
    OUT_ERR=$(echo -n "" | "$BIN" --ags --led 2>/dev/null | head -1)
    if echo "$OUT_ERR" | grep -q "⚠\|cava\|error"; then
        pass "EOF inmediato → produce markup de error"
    else
        fail "EOF inmediato NO produce markup de error — salida: $OUT_ERR"
    fi

    # El flag --eww sigue funcionando como alias
    OUT_ALIAS=$(cava_frames 1 16383 | "$BIN" --eww --led 2>/dev/null | head -1)
    if echo "$OUT_ALIAS" | grep -q "<span"; then
        pass "--eww sigue funcionando como alias de --ags"
    else
        fail "--eww no funciona como alias — salida: $OUT_ALIAS"
    fi

    # Pico alto → clase 'peak' en JSON
    OUT_PEAK=$(cava_frames 1 32767 | "$BIN" --led 2>/dev/null | head -1)
    if echo "$OUT_PEAK" | grep -q '"peak"'; then
        pass "Amplitud máxima → clase CSS 'peak' en JSON"
    else
        fail "Amplitud máxima NO genera clase 'peak' — salida: $OUT_PEAK"
    fi
fi

# ── Bloque 6: Pipeline completo ──────────────────────────────────────────────

echo ""
echo -e "  ${BOLD}Pipeline completo${NC}"

if ! command -v cava &>/dev/null || ! [ -f "$HOME/.config/cava/ags.ini" ] || ! [ -f "$BIN" ]; then
    skip "Pipeline no testeable (faltan dependencias)"
else
    PIPE_OUT=$(timeout 3s bash -c \
        "cava -p \"$HOME/.config/cava/ags.ini\" | \"$BIN\" --ags --led" \
        2>/dev/null | head -1 || true)

    if echo "$PIPE_OUT" | grep -qE "<span|▁"; then
        pass "Pipeline cava | ags-cavars produce salida válida"
    else
        fail "Pipeline cava | ags-cavars no produce salida — prueba manualmente:"
        fail "  cava -p ~/.config/cava/ags.ini | ags-cavars --ags --led | head -3"
    fi
fi

# ── Resumen ───────────────────────────────────────────────────────────────────

echo ""
echo "  ───────────────────────────────"
TOTAL=$((PASS + FAIL + SKIP))
echo -e "  ${BOLD}Resultado:${NC} $TOTAL tests — ${GREEN}$PASS OK${NC} · ${RED}$FAIL fallidos${NC} · ${YELLOW}$SKIP omitidos${NC}"
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo -e "  ${RED}Hay fallos. Lee los mensajes de arriba para solucionarlos.${NC}"
    echo ""
    exit 1
else
    echo -e "  ${GREEN}Todo en orden. El pipeline está listo.${NC}"
    echo ""
    exit 0
fi
