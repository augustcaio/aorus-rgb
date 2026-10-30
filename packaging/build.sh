#!/usr/bin/env bash
# Gera os artefatos de uma release a partir do conteúdo exato de uma tag, fora
# do repositório (nenhum arquivo gerado aparece na pasta do projeto):
#   aorus_rgb-<versão>-py3-none-any.whl
#   aorus-rgb-<versão>-<pkgrel>-any.pkg.tar.zst        (daemon e CLI)
#   aorus-rgb-gui-<versão>-<pkgrel>-x86_64.pkg.tar.zst (interface GTK, em gui/)
#
# Uso: packaging/build.sh vX.Y.Z [pasta-de-saída]
# Requer: base-devel python-build python-installer python-pytest cargo
#         gtk4 libadwaita (namcap opcional)
set -euo pipefail

REPO="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
TAG="${1:?uso: packaging/build.sh vX.Y.Z [pasta-de-saída]}"
OUT="${2:-$HOME/.cache/aorus-rgb/dist/$TAG}"

git -C "$REPO" rev-parse -q --verify "refs/tags/$TAG" >/dev/null || { echo "erro: tag $TAG não existe" >&2; exit 1; }
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT"

git -C "$REPO" archive --prefix=src/ "$TAG" | tar -x -C "$WORK"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$WORK/src/pyproject.toml")"
pkgver="$(sed -n 's/^pkgver=//p' "$WORK/src/packaging/arch/PKGBUILD")"
gui_version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$WORK/src/gui/Cargo.toml" | head -n1)"
gui_pkgver="$(sed -n 's/^pkgver=//p' "$WORK/src/packaging/arch-gui/PKGBUILD")"
for v in "$version" "$pkgver" "$gui_version" "$gui_pkgver"; do
    if [[ "v$v" != "$TAG" ]]; then
        echo "erro: $TAG não bate com pyproject ($version), PKGBUILD ($pkgver)," \
             "gui/Cargo.toml ($gui_version) e PKGBUILD da interface ($gui_pkgver)" >&2
        exit 1
    fi
done

echo "==> wheel"
python -m build --wheel --no-isolation --outdir "$OUT" "$WORK/src"

echo "==> pacote do Arch"
# clona a tag do repositório local (o caminho do projeto tem espaço e acento,
# que o makepkg não aceita numa URL git+file)
git clone -q --bare "$REPO" "$WORK/aorus-rgb.git"
local_source() {
    sed "s|git+https://github.com/augustcaio/aorus-rgb.git|git+file://$WORK/aorus-rgb.git|" "$1" > "$2"
}
mkdir "$WORK/arch" "$WORK/arch-gui"
local_source "$WORK/src/packaging/arch/PKGBUILD" "$WORK/arch/PKGBUILD"
cp "$WORK/src/packaging/arch/aorus-rgb.install" "$WORK/arch/"
(cd "$WORK/arch" && PKGDEST="$OUT" SRCDEST="$WORK/srcdest" BUILDDIR="$WORK/build" makepkg --cleanbuild --noconfirm)

echo "==> pacote da interface"
# --nodeps: depende do aorus-rgb desta mesma versão, gerado logo acima e
# ainda não instalado
local_source "$WORK/src/packaging/arch-gui/PKGBUILD" "$WORK/arch-gui/PKGBUILD"
(cd "$WORK/arch-gui" && PKGDEST="$OUT" SRCDEST="$WORK/srcdest" BUILDDIR="$WORK/build" makepkg --cleanbuild --noconfirm --nodeps)

if command -v namcap >/dev/null; then
    echo "==> namcap"
    for pkg in "$OUT"/*.pkg.tar.zst; do namcap "$pkg" || true; done
fi

(cd "$OUT" && sha256sum ./*.whl ./*.pkg.tar.zst > SHA256SUMS)
echo "==> artefatos em $OUT"
ls -l "$OUT"
