# ADR 0001 — Reescrita do Backend em Rust com Axum e SQLx

## Status
Aceito e Concluído.

## Contexto
O backend legado do BitcoSats era implementado em TypeScript com Node.js, Express e Prisma ORM. O sistema sofria com:
1. **Riscos de Concorrência**: Prisma ORM abstrai transações e locks pessimistas de forma opaca, facilitando race conditions financeiras.
2. **Consumo de Recursos**: Node.js apresentava alto uso de memória e latências imprevisíveis em momentos de pico.
3. **Segurança de Tipos Numéricos**: Tipos nativos de ponto flutuante em JavaScript exigiam manuseio cuidadoso de bibliotecas como `bignumber.js`, com risco constante de erros de arredondamento.

## Decisão
Reescrever o backend inteiramente em **Rust**, utilizando:
- **Axum** para a camada HTTP e roteamento.
- **SQLx** para comunicação direta com PostgreSQL em tempo de compilação, com total controle sobre transações, locks e isolamento.
- **Tokio** para o runtime assíncrono do servidor e do worker.
- **BigDecimal / i128** para precisão financeira estrita em todas as moedas.

## Consequências
- Garantia de segurança de memória e ausência de *data races* em tempo de compilação.
- Redução drástica do footprint de memória e latência inferior a 5ms nas operações centrais.
- Facilidade de deploy em containers leves (< 30MB) e sem runtime de terceiros.
