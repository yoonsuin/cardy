#!/usr/bin/env bash
# ============================================================
#  Release Publisher Board – 로컬 빌드 스크립트 (macOS)
#  실행: bash build-local.sh
# ============================================================
set -euo pipefail

echo "═══════════════════════════════════════════════════════"
echo "  Release Publisher Board – 로컬 빌드"
echo "═══════════════════════════════════════════════════════"

# ── 필수 도구 확인 ────────────────────────────────────────
for cmd in pnpm cargo rustup; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "❌ '$cmd' 가 설치되어 있지 않습니다."
    case "$cmd" in
      pnpm)    echo "   → npm install -g pnpm" ;;
      cargo)   echo "   → https://rustup.rs 에서 Rust 설치" ;;
      rustup)  echo "   → https://rustup.rs 에서 Rust 설치" ;;
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

# ── 아키텍처 선택 ─────────────────────────────────────────
ARCH=$(uname -m)
echo ""
echo "🖥  감지된 아키텍처: $ARCH"

if [[ "$ARCH" == "arm64" ]]; then
  RUST_TARGET="aarch64-apple-darwin"
else
  RUST_TARGET="x86_64-apple-darwin"
fi

# Rust 타겟 추가
rustup target add "$RUST_TARGET" 2>/dev/null || true

# ── Tauri 빌드 ────────────────────────────────────────────
echo ""
echo "🔨 Tauri 빌드 시작 (target: $RUST_TARGET)..."
pnpm tauri build --target "$RUST_TARGET"

# ── 결과물 경로 안내 ──────────────────────────────────────
echo ""
echo "═══════════════════════════════════════════════════════"
echo "  ✅ 빌드 완료!"
echo ""
echo "  📁 산출물 위치:"
echo "  src-tauri/target/$RUST_TARGET/release/bundle/"
echo ""
echo "  macOS 앱:  bundle/macos/Release Publisher Board.app"
echo "  DMG 파일:  bundle/dmg/*.dmg"
echo "═══════════════════════════════════════════════════════"
