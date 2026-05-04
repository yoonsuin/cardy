# Cardy

> Jira 이슈를 카드뉴스로 변환해 Slack에 바로 발행하는 로컬 데스크탑 앱

[![Build](https://github.com/yoonsuin/cardy/actions/workflows/build.yml/badge.svg)](https://github.com/yoonsuin/cardy/actions/workflows/build.yml)

---

## 다운로드

👉 **[cardy 다운로드 페이지](https://yoonsuin.github.io/cardy/)**

| 플랫폼 | 지원 버전 |
|---|---|
| macOS Apple Silicon (M1/M2/M3) | macOS 11 Big Sur 이상 |
| macOS Intel (x86_64) | macOS 11 Big Sur 이상 |
| Windows 64-bit | Windows 10 / 11 |

---

## 기능

- **Jira 연동** — JQL 쿼리로 이슈를 불러와 카드뉴스 형태로 시각화
- **Slack 발행** — 카드 이미지를 원하는 채널에 바로 업로드
- **로컬 우선** — API 토큰은 로컬 디바이스에만 저장, 외부 서버 전송 없음
- **크로스 플랫폼** — macOS / Windows 네이티브 앱

---

## 개발 환경 설정

### 필수 도구

- [Node.js](https://nodejs.org/) 22 이상
- [pnpm](https://pnpm.io/) 10 이상
- [Rust](https://rustup.rs/) stable

### 실행

```bash
# 의존성 설치
pnpm install

# 개발 서버 실행
pnpm tauri:dev
```

### 빌드

```bash
# 로컬 빌드 (macOS)
bash build-local.sh
```

---

## 릴리즈

태그를 push하면 GitHub Actions가 자동으로 macOS / Windows 바이너리를 빌드하고 Release에 업로드합니다.

```bash
git tag v0.1.0
git push origin v0.1.0
```

---

## 기술 스택

| 영역 | 기술 |
|---|---|
| Frontend | React 19 + TypeScript + Vite |
| Desktop | Tauri 2 (Rust) |
| API 연동 | Jira REST API v3, Slack Web API |

---

## 라이선스

MIT © [yoonsuin](https://github.com/yoonsuin)
