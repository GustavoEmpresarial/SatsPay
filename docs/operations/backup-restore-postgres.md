# Backup e Recuperação Point-in-Time do PostgreSQL (PITR)

O cluster PostgreSQL em produção utiliza o operador **CloudNativePG** (`third-party/cloudnative-pg/cluster-prod.yaml`) com o motor **Barman** integrado para arquivamento contínuo de logs WAL e backups periódicos para armazenamento compatível com S3 (AWS S3, MinIO, Cloudflare R2).

---

## 1. Topologia de Backup

1. **Arquivamento Contínuo de WAL**: Cada segmento de Write-Ahead Log é enviado imediatamente ao bucket S3 configurado, garantindo um Recovery Point Objective (RPO) próximo de zero.
2. **Backups Físicos Completos (Snapshots)**: Executados diariamente via `ScheduledBackup` do Kubernetes.

---

## 2. Testando o Restore com `exercise-pitr-restore.sh`

O repositório inclui um script automatizado de teste de Point-in-Time Recovery em `scripts/exercise-pitr-restore.sh`.

```bash
# Executar teste de restore no cluster k8s
./scripts/exercise-pitr-restore.sh
```

### O que o script realiza:
1. Conecta no cluster principal e insere uma linha canário de teste com timestamp conhecido.
2. Força o flush dos WALs atuais para o bucket de backup (`pg_switch_wal`).
3. Aguarda o arquivamento no S3.
4. Cria um cluster efêmero de recuperação (`Cluster` CR com diretiva `recovery.source`) apontando para o timestamp exato anterior.
5. Verifica se o cluster restaurado subiu perfeitamente e se os dados e o registro canário conferem.
6. Limpa o cluster de teste.

---

## 3. Procedimento Manual de Recuperação em Desastre (Disaster Recovery)

Em caso de corrupção ou falha catastrófica no cluster principal:

1. **Definir o Timestamp Alvo de Recuperação**: Exemplo: `2026-09-01 12:00:00.000000+00`.
2. **Aplicar o Manifest de Recuperação**:
   ```yaml
   apiVersion: postgresql.cnpg.io/v1
   kind: Cluster
   metadata:
     name: bitcosats-postgres-recovered
     namespace: bitcosats
   spec:
     instances: 3
     bootstrap:
       recovery:
         source: bitcosats-postgres-backup-s3
         targetTime: "2026-09-01T12:00:00Z"
     storage:
       size: 50Gi
   ```
3. **Redirecionar a Aplicação**: Atualizar a referência de secret `DATABASE_URL` para o novo cluster `bitcosats-postgres-recovered-rw`.

---

## 4. Stack Compose na VM (produção atual)

Enquanto o CNPG/Barman não estiver no ar nesta VM, o DR exercitável é dump/restore Docker:

```bash
./scripts/exercise_compose_pg_backup_restore.sh
```

O script: (1) insere canário em `bitcosats-postgres`, (2) `pg_dump -Fc`, (3) sobe Postgres 16 efêmero, (4) `pg_restore`, (5) confere o canário, (6) limpa. Não substitui PITR contínuo com WAL/S3 — só prova que o backup lógico do compose funciona.
