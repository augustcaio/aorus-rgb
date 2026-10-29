#!/usr/bin/env bash
# Cria o ambiente de desenvolvimento fora do repositório, sem gerar
# arquivos na pasta do projeto (sem egg-info, sem __pycache__).
#
#   venv      -> ~/.venvs/aorus-rgb
#   bytecode  -> ~/.cache/aorus-rgb/pycache
#   ruff      -> ~/.cache/aorus-rgb/ruff (configurado no pyproject.toml)
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV="${AORUS_RGB_VENV:-$HOME/.venvs/aorus-rgb}"
CACHE="$HOME/.cache/aorus-rgb"

python -m venv "$VENV"
"$VENV/bin/pip" install -q --upgrade "pytest>=8" "ruff>=0.6"

SITE="$("$VENV/bin/python" -c 'import sysconfig; print(sysconfig.get_path("purelib"))')"

# o código é importado direto do repositório, sem instalar o pacote
echo "$REPO" > "$SITE/aorus_rgb_dev.pth"

# bytecode fora da pasta do projeto
cat > "$SITE/sitecustomize.py" <<EOF
import sys
sys.pycache_prefix = "$CACHE/pycache"
EOF

# comando aorus-rgb
cat > "$VENV/bin/aorus-rgb" <<EOF
#!$VENV/bin/python
from aorus_rgb.__main__ import main
main()
EOF
chmod +x "$VENV/bin/aorus-rgb"

mkdir -p "$CACHE"
echo "ok: source $VENV/bin/activate"
