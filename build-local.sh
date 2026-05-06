#!/usr/bin/env bash
# ============================================================
#  Cardy – 로컬 빌드 & 실행 스크립트 (macOS)
#  실행: bash build-local.sh
# ============================================================
set -euo pipefail

cd "$(dirname "$0")"

echo "═══════════════════════════════════════════════════════"
echo "  Cardy – 로컬 빌드"
echo "═══════════════════════════════════════════════════════"

# ── 필수 도구 확인 ────────────────────────────────────────
for cmd in pnpm cargo rustup; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "❌ '$cmd' 가 설치되어 있지 않습니다."
    case "$cmd" in
      pnpm)   echo "   → npm install -g pnpm" ;;
      cargo|rustup) echo "   → https://rustup.rs 에서 Rust 설치" ;;
    esac
    exit 1
  fi
done
echo "✅ 필수 도구 확인 완료"

# ── Node 패키지 설치 ──────────────────────────────────────
echo ""
echo "📦 Node 패키지 설치 중..."
pnpm install

# ── 아이콘 생성 ───────────────────────────────────────────
echo ""
echo "🎨 앱 아이콘 생성 중..."
pnpm tauri icon src-tauri/icons/icon.png

# ── 아키텍처 자동 감지 ────────────────────────────────────
ARCH=$(uname -m)
echo ""
echo "🖥  감지된 아키텍처: $ARCH"
if [[ "$ARCH" == "arm64" ]]; then
  RUST_TARGET="aarch64-apple-darwin"
else
  RUST_TARGET="x86_64-apple-darwin"
fi
rustup target add "$RUST_TARGET" 2>/dev/null || true

# ── 개발 모드 vs 릴리즈 모드 선택 ────────────────────────
echo ""
echo "어떤 빌드를 원하시나요?"
echo "  1) dev   – 빠른 빌드, 개발자 도구 포함 (테스트용 권장)"
echo "  2) build – 최적화된 릴리즈 빌드 (배포용)"
echo ""
read -rp "선택 [1/2, 기본값 1]: " CHOICE
CHOICE="${CHOICE:-1}"

if [[ "$CHOICE" == "2" ]]; then
  echo ""
  echo "🔨 릴리즈 빌드 시작 (target: $RUST_TARGET)..."
  pnpm tauri build --target "$RUST_TARGET"

  echo ""
  echo "═══════════════════════════════════════════════════════"
  echo "  ✅ 릴리즈 빌드 완료!"
  echo ""
  BUNDLE="src-tauri/target/$RUST_TARGET/release/bundle"
  echo "  📁 산출물 위치: $BUNDLE"
  echo "  macOS 앱:  $BUNDLE/macos/Cardy.app"
  echo "  DMG 파일:  $BUNDLE/dmg/*.dmg"
  echo ""
  echo "  DMG를 열어 Applications 폴더로 드래그하세요."
  echo "  최초 실행 시 Gatekeeper 경고가 뜨면:"
  echo "  우클릭 → 열기  또는"
  echo "  시스템 설정 → 개인정보 보호 및 보안 → 확인 없이 열기"
  echo "═══════════════════════════════════════════════════════"
else
  echo ""
  echo "🚀 개발 서버 시작 중..."
  echo "  (앱 창이 열립니다. 종료하려면 Ctrl+C)"
  echo ""
  pnpm tauri dev
fi
