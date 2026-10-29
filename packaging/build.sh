#!/usr/bin/env bash
# Gera os artefatos de uma release a partir do conteúdo exato de uma tag, fora
# do repositório (nenhum arquivo gerado aparece na pasta do projeto):
#   aorus_rgb-<versão>-py3-none-any.whl
#   aorus-rgb-<versão>-<pkgrel>-any.pkg.tar.zst
#
# Uso: packaging/build.sh vX.Y.Z [pasta-de-saída]
# Requer: base-devel python-build python-installer python-pytest (namcap opcional)
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
if [[ "v$version" != "$TAG" || "v$pkgver" != "$TAG" ]]; then
    echo "erro: $TAG, pyproject ($version) e PKGBUILD ($pkgver) não batem" >&2
    exit 1
fi

echo "==> wheel"
python -m build --wheel --no-isolation --outdir "$OUT" "$WORK/src"

echo "==> pacote do Arch"
# clona a tag do repositório local (o caminho do projeto tem espaço e acento,
# que o makepkg não aceita numa URL git+file)
git clone -q --bare "$REPO" "$WORK/aorus-rgb.git"
mkdir "$WORK/arch"
sed "s|git+https://github.com/augustcaio/aorus-rgb.git|git+file://$WORK/aorus-rgb.git|" \
    "$WORK/src/packaging/arch/PKGBUILD" > "$WORK/arch/PKGBUILD"
cp "$WORK/src/packaging/arch/aorus-rgb.install" "$WORK/arch/"
(cd "$WORK/arch" && PKGDEST="$OUT" SRCDEST="$WORK/srcdest" BUILDDIR="$WORK/build" makepkg --cleanbuild --noconfirm)

if command -v namcap >/dev/null; then
    echo "==> namcap"
    namcap "$OUT"/aorus-rgb-*.pkg.tar.zst || true
fi

(cd "$OUT" && sha256sum ./*.whl ./*.pkg.tar.zst > SHA256SUMS)
echo "==> artefatos em $OUT"
ls -l "$OUT"
