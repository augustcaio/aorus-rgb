#!/usr/bin/env bash
# Instala (ou remove, com --uninstall) o serviço de usuário do aorus-rgb a partir
# do código de desenvolvimento. Com o pacote do Arch, use o serviço dele.
# Não precisa de sudo: a unidade fica em ~/.config/systemd/user.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
UNIT=aorus-rgb.service

if [[ "${1:-}" == "--uninstall" ]]; then
    systemctl --user disable --now "$UNIT" 2>/dev/null || true
    rm -f "$UNIT_DIR/$UNIT"
    systemctl --user daemon-reload
    echo "serviço removido"
    exit 0
fi

if [[ ! -x "$HOME/.venvs/aorus-rgb/bin/aorus-rgb" ]]; then
    echo "erro: rode ./dev-setup.sh antes (o serviço usa ~/.venvs/aorus-rgb)" >&2
    exit 1
fi

if [[ -f "/usr/lib/systemd/user/$UNIT" ]]; then
    echo "aviso: o pacote aorus-rgb está instalado; esta cópia de desenvolvimento" >&2
    echo "       tem prioridade sobre o serviço do pacote até --uninstall" >&2
fi

# mesma unidade do pacote, rodando o código do venv de desenvolvimento
mkdir -p "$UNIT_DIR"
sed "s|/usr/bin/aorus-rgb|$HOME/.venvs/aorus-rgb/bin/aorus-rgb|" "$REPO/$UNIT" > "$UNIT_DIR/$UNIT"
chmod 644 "$UNIT_DIR/$UNIT"
systemctl --user daemon-reload
systemctl --user enable --now "$UNIT"
systemctl --user --no-pager --lines=0 status "$UNIT" || true
