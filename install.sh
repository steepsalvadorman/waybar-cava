#!/bin/bash
set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ok()   { echo -e "${GREEN}✓${NC} $1"; }
warn() { echo -e "${YELLOW}!${NC} $1"; }
die()  { echo -e "${RED}✗${NC} $1"; exit 1; }

echo ""
echo "  ags-cavars · instalador"
echo "  ─────────────────────────"
echo ""

# Dependencias
command -v cargo &>/dev/null || die "Rust no está instalado. Instálalo en https://rustup.rs"
command -v cava  &>/dev/null || die "CAVA no está instalado. Instálalo con: sudo pacman -S cava"
ok "Dependencias OK"

# Compilar
echo ""
echo "  Compilando..."
RUSTFLAGS="-A warnings" cargo build --release -q 2>/dev/null
ok "Binario compilado"

# Instalar binario
mkdir -p "$HOME/.local/bin"
cp target/release/ags-cavars "$HOME/.local/bin/ags-cavars"
ok "Binario instalado en ~/.local/bin/ags-cavars"

# Instalar config de CAVA
mkdir -p "$HOME/.config/cava"
if [ -f "$HOME/.config/cava/ags.ini" ]; then
    warn "~/.config/cava/ags.ini ya existe, se conserva el tuyo"
else
    cp config/ags.ini "$HOME/.config/cava/ags.ini"
    ok "Config de CAVA instalado en ~/.config/cava/ags.ini"
fi

# Comprobar PATH
if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
    warn "~/.local/bin no está en tu PATH"
    echo "     Añade esto a tu ~/.zshrc o ~/.bashrc:"
    echo "     export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

echo ""
echo "  ─────────────────────────"
echo "  Verificando instalación..."
echo ""

if bash "$(dirname "$0")/test.sh"; then
    echo "  Listo. Añade esto en tu Right.tsx de AGS:"
    echo ""
    echo '  subprocess(['
    echo '    "bash", "-c",'
    echo '    `cava -p ${HOME}/.config/cava/ags.ini | ${HOME}/.local/bin/ags-cavars --ags --led`'
    echo '  ], (line) => setCava(line))'
    echo ""
else
    echo ""
    warn "La instalación tiene problemas. Lee los mensajes de arriba."
    exit 1
fi
