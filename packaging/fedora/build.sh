#!/usr/bin/env bash
# Gera os RPMs do Fedora a partir do conteúdo exato de uma tag, num container
# fedora (docker ou podman), fora do repositório:
#   aorus-rgb-<versão>-<release>.fcNN.noarch.rpm
#   aorus-rgb-gui-<versão>-<release>.fcNN.x86_64.rpm
#   e os .src.rpm, para o COPR ou para reconstruir
#
# Uso: packaging/fedora/build.sh vX.Y.Z [pasta-de-saída] [imagem]
# Imagem padrão: registry.fedoraproject.org/fedora:45 (RPMs .fc45)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(git -C "$HERE" rev-parse --show-toplevel)"
TAG="${1:?uso: packaging/fedora/build.sh vX.Y.Z [pasta-de-saída] [imagem]}"
OUT="${2:-$HOME/.cache/aorus-rgb/dist/fedora/$TAG}"
IMAGE="${3:-registry.fedoraproject.org/fedora:45}"
ENGINE="$(command -v podman || command -v docker)" || { echo "erro: precisa de podman ou docker" >&2; exit 1; }

git -C "$REPO" rev-parse -q --verify "refs/tags/$TAG" >/dev/null || { echo "erro: tag $TAG não existe" >&2; exit 1; }
version="${TAG#v}"
for spec in "$HERE"/*.spec; do
    v="$(sed -n 's/^Version:[[:space:]]*//p' "$spec")"
    [[ "$v" == "$version" ]] || { echo "erro: $(basename "$spec") tem Version $v, a tag é $TAG" >&2; exit 1; }
done

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$OUT" "$WORK/SOURCES" "$WORK/SPECS"
# mesmo nome e prefixo do arquivo que o GitHub gera para a tag (Source0)
git -C "$REPO" archive --prefix="aorus-rgb-$version/" -o "$WORK/SOURCES/aorus-rgb-$version.tar.gz" "$TAG"
cp "$HERE"/*.spec "$WORK/SPECS/"

"$ENGINE" run --rm -e LANG=C.UTF-8 -v "$WORK:/work:Z" -v "$OUT:/out:Z" "$IMAGE" bash -euo pipefail -c '
    dnf -y -q install rpm-build rpmlint "dnf-command(builddep)" >/dev/null
    top=(--define "_topdir /work")
    for spec in /work/SPECS/aorus-rgb.spec /work/SPECS/aorus-rgb-gui.spec; do
        echo "==> $(basename "$spec")"
        dnf -y -q builddep "$spec" >/dev/null
        # dependências geradas (%generate_buildrequires): resolve até estabilizar
        until rpmbuild "${top[@]}" -br "$spec" >/dev/null 2>&1; do
            dnf -y -q builddep /work/SRPMS/*.buildreqs.nosrc.rpm >/dev/null
            rm -f /work/SRPMS/*.buildreqs.nosrc.rpm
        done
        rm -f /work/SRPMS/*.buildreqs.nosrc.rpm
        rpmbuild "${top[@]}" -ba "$spec"
    done
    echo "==> rpmlint"
    rpmlint /work/RPMS/*/*.rpm /work/SRPMS/*.src.rpm || true
    cp /work/RPMS/*/*.rpm /work/SRPMS/*.src.rpm /out/
    # arquivos criados como root no container: devolve ao usuário (saída e limpeza)
    chown -R '"$(id -u):$(id -g)"' /out /work
'

(cd "$OUT" && sha256sum ./*.rpm > SHA256SUMS)
echo "==> artefatos em $OUT"
ls -l "$OUT"
