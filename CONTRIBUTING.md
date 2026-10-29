# Convenções

## Estrutura de branches
- **`main`**: somente a pasta `docs/`, os arquivos `.md` e a licença. Sem código.
- **Branch de feature** (`feat/<nome>`): contém somente o código da feature.
  Nasce **vazia** (branch órfã, sem herdar nada da `main`), e não carrega docs,
  `.md` nem licença.
- **Exceção**: uma feature que é fork, continuação ou ramificação de outra
  feature existente nasce a partir dela, herdando o código dela.
- Toda feature nova entra na tabela de branches do `README.md` (na `main`).

Criar uma feature nova:

    git switch --orphan feat/<nome>

## Commits (Conventional Commits)
Mensagens de commit são escritas **em inglês**, no imperativo:

    type(scope): short description

    Corpo opcional explicando o porquê da mudança.

Tipos: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `chore`.
Escopos usados: `device`, `cli`, `udev`, `smbus`, `docs`.

Exemplos:
- `feat(cli): add rainbow command`
- `fix(udev): use 70- prefix so uaccess is applied`

Os commits têm um único autor (`augustcaio`), sem trailers `Co-Authored-By`.

## Versões
- Cada versão estável ganha uma branch `release/X.Y.Z`, criada a partir da
  feature que reúne o código validado, com a versão do `pyproject.toml`
  atualizada e a tag `vX.Y.Z` no commit `chore(release): X.Y.Z`.
- A branch de release não recebe desenvolvimento; correções numa versão
  publicada viram `release/X.Y.Z+1` (patch).
- A cada release, a seção `[Unreleased]` do `CHANGELOG.md` vira `[X.Y.Z]`, e a
  release entra na tabela de branches do `README.md`.

### Publicar uma release
1. Na branch `release/X.Y.Z`: versão no `pyproject.toml` e `pkgver` no
   `packaging/arch/PKGBUILD`, commit `chore(release): X.Y.Z` e tag `vX.Y.Z`
   (ainda só local).
2. `packaging/build.sh vX.Y.Z` gera wheel, pacote e `SHA256SUMS`; instalar o
   pacote e validar no hardware.
3. Enviar branch e tag, e criar a release no GitHub com os três arquivos.
4. AUR: copiar `PKGBUILD` e `aorus-rgb.install` para o repositório do AUR,
   gerar o `.SRCINFO` (`makepkg --printsrcinfo > .SRCINFO`) e enviar.

O pacote `aorus-rgb` ainda não está no AUR: o cadastro de contas novas está
suspenso pelo próprio AUR. O nome estava livre em 2026-10-29 e o `PKGBUILD` da
1.1.0 já constrói a partir da tag no GitHub.
