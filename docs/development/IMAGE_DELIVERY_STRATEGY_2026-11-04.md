# Bionic fork image delivery — Personal Expert AI

Дата: 2026-11-04  
Решение: **вариант C — local k3d now, private GHCR-ready architecture**.

## Цель

Локальный Bionic deployment должен запускать именно fork `Dach1980/bionic-personal-expert-ai`, включая серверный PEA Adapter. Использование upstream `ghcr.io/bionic-gpt/bionicgpt:1.13.4` не является допустимым подтверждением работы форка.

## Принятое решение

1. Сейчас собираем fork-specific Docker image из проверенного commit SHA.
2. Передаём образ непосредственно в локальный k3d; GHCR не является обязательной зависимостью.
3. В manifests используем параметризуемое имя образа и неизменяемый tag/digest.
4. Позже добавляем публикацию того же проверенного image в private GHCR.
5. Переключение local image ↔ GHCR не должно менять код PEA Adapter или runtime configuration приложения.

Пример соглашения об именовании (пока проектное, не фактический опубликованный образ):

- local: `personal-expert-ai/bionic:<full-or-short-commit-sha>`
- private registry: `ghcr.io/dach1980/bionic-personal-expert-ai:<full-or-short-commit-sha>`

Для release provenance фиксировать digest, commit SHA, workflow run и результаты тестов. Не использовать `latest` как идентификатор принятой сборки.

## Аудит текущего состояния

- `infra-as-code/stack.yaml` указывает web service на upstream `ghcr.io/bionic-gpt/bionicgpt:1.13.4`.
- `infra-as-code/docker-compose.yml` также указывает app на upstream image.
- Остальные образы (DB migrations, cron, RAG engine и вспомогательные сервисы) пока остаются upstream. Пересобирать их без доказанной необходимости не нужно.
- Текущий Rust/UI workflow проверяет форматирование, compilation, Rust tests и web assets, но не строит fork-specific production image.
- Успешный Rust CI поэтому не подтверждает, что runtime контейнер содержит PEA Adapter.
- `dev-init.sh` удаляет существующий `k3d-bionic` cluster и создаёт его заново. Не запускать автоматически.
- `get-config.sh` изменяет kubeconfig и отключает TLS verification для локальной разработки. Это не production конфигурация.

## Реализационные задачи

### A. Docker packaging discovery

- Найти и зафиксировать upstream Docker build context и точную последовательность сборки.
- Проверить, как собираются Rust web server и статические assets.
- Проверить entrypoint, runtime dependencies, certificates, non-root user и health endpoint.
- Убедиться, что в production image попадает именно fork binary, а не только исходники/страница Pages.
- Зафиксировать build args и необходимые файлы без секретов.

**Приёмка:** повторяемая инструкция сборки с commit provenance; локальная сборка не требует GHCR.

### B. Local image

- Добавить Dockerfile/build target или воспроизводимый packaging workflow на основе upstream-процесса.
- Тегировать image commit SHA.
- Проверять наличие PEA Adapter routes в собранном приложении.
- Проверять health endpoint и image metadata.
- Не включать пользовательские документы, модели, токены и локальные конфигурации в build context.

**Приёмка:** image build job проходит в GitHub Actions; образ содержит проверенный fork code.

### C. k3d deployment manifests

- Вынести web image reference в единую конфигурацию.
- Поддержать local image name и GHCR image name без изменения приложения.
- Обновить Stack manifest; Compose держать согласованным для альтернативного dev workflow.
- Не менять upstream DB migrations/RAG/cron images без отдельного обоснования.
- Предусмотреть imagePullPolicy для локального образа и imagePullSecrets только для private GHCR profile.

**Приёмка:** manifest validation проходит в CI; выбранный image reference однозначен.

### D. Runtime verification (blocked until local environment access)

- Собрать image на машине с Docker.
- Импортировать его в существующий k3d cluster, не удаляя кластер.
- Развернуть Stack update.
- Проверить image ID/digest запущенного pod.
- Проверить наличие Adapter routes и фактическую работу JWT validation.
- Проверить, что Bionic → PEA соединение идёт по private path.

**Приёмка:** runtime evidence содержит commit SHA, digest, pod imageID и результат smoke tests.

### E. Private GHCR (later)

- Настроить package visibility private.
- Добавить least-privilege GitHub Actions permissions и OIDC/package publishing.
- Публиковать только после image build + tests.
- Подписывать/аттестовать image provenance и сохранять digest.
- Настроить private registry pull credentials для deployment environment.
- Не делать GHCR обязательным для local k3d developer loop.

## Ограничения текущего режима

Пока пользователь не может запускать локальные Docker/k3d команды, выполняются repository-only работы: Docker packaging audit, Dockerfile/build workflow, manifest validation, unit/contract tests и документация. Нельзя объявлять локальную сборку, импорт в k3d, pod rollout или реальный Adapter smoke test завершёнными.

## Связанный межрепозиторный план

См. [Personal Expert AI implementation roadmap](https://github.com/Dach1980/Personal_Expert_AI/blob/main/docs/development/IMPLEMENTATION_ROADMAP_2026-11-04.md).
