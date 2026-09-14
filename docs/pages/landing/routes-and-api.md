# Landing — Rotas e API

## Client

| Path | Auth | Resultado |
|------|------|-----------|
| `/` | — | Landing ou redirect dashboard |
| `/welcome` | — | Landing |

Não chama endpoints REST no mount.

## Segurança

| Tema | Comportamento |
|------|----------------|
| Sessão | User autenticado não vê marketing em `/` |
| XSS | Conteúdo i18n estático |
