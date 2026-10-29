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
`tipo(escopo): descrição no imperativo`

Tipos: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `chore`.
Escopos usados: `device`, `cli`, `udev`, `smbus`, `docs`.

Exemplos:
- `feat(cli): adicionar comando rainbow`
- `fix(udev): usar prefixo 70 para aplicar uaccess`

Os commits têm um único autor (`augustcaio`), sem trailers `Co-Authored-By`.

## Versões
Tags `vMAJOR.MINOR.PATCH`; atualizar o `CHANGELOG.md` a cada release.
