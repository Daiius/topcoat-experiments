#!/usr/bin/env bash
# topcoat 版の本番イメージを linux/amd64 でビルドし、起動確認のうえ ghcr.io へ push する（VPS 配置用）。
#
#   ./scripts/image-topcoat.sh --check   # 何もせず、使う値と状態だけ表示
#   ./scripts/image-topcoat.sh           # ビルド + 起動確認（push しない）
#   ./scripts/image-topcoat.sh --push    # ビルド + 起動確認 + push（YES=1 で確認を省く）
#
# 環境変数:
#   IMAGE=ghcr.io/daiius/topcoat-experiments-topcoat   push 先（タグは付けない）
#   DIRTY_OK=1   ビルドに入るファイルに未コミットの変更があっても続ける
#   YES=1        push 前の確認を省く（Claude Code の `!` 実行など、入力できない場合）
#
# タグは git の短いハッシュと latest。VPS ではハッシュのタグを指定すると切り戻しやすい。
# ⚠ ghcr.io に初めて push したパッケージは private になる。VPS から pull するには、
#   GitHub のパッケージ設定で public にするか、VPS で `docker login ghcr.io` する。
# ⚠ org.opencontainers.image.source ラベルは付けない。付けると ghcr が初回 push 時にパッケージを
#   リポジトリへ紐付けようとする。付けていた時は、層は上がったうえで最後に `denied` になった
#   （2026-11-04。ラベルの無い他のイメージは同じ認証で push できている）。
set -euo pipefail

IMAGE="${IMAGE:-ghcr.io/daiius/topcoat-experiments-topcoat}"
PLATFORM="linux/amd64"
MODE=build
case "${1:-}" in
  --check) MODE=check ;;
  --push) MODE=push ;;
  "") ;;
  *) echo "不明な引数: $1（--check / --push）" >&2; exit 2 ;;
esac

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
REV="$(git rev-parse --short HEAD)"
# ビルドが読むもの（.dockerignore で絞った後）: topcoat.Dockerfile, data/, topcoat/
DIRTY="$(git status --porcelain -- topcoat.Dockerfile .dockerignore data topcoat | grep -v '^??' || true)"
REGISTRY="${IMAGE%%/*}"

echo "image:    $IMAGE:$REV（と :latest）"
echo "platform: $PLATFORM"
echo "commit:   $(git log --oneline -1)"
if [ -n "$DIRTY" ]; then echo "未コミットの変更:"; echo "$DIRTY" | sed 's/^/  /'; else echo "未コミットの変更: なし"; fi
if python3 -c "import json,os,sys;sys.exit(0 if '$REGISTRY' in json.load(open(os.path.expanduser('~/.docker/config.json'))).get('auths',{}) else 1)" 2>/dev/null; then
  echo "login:    $REGISTRY にログイン済み（push 権限までは確認していない）"
else
  echo "login:    $REGISTRY にログインしていない（push には docker login が要る）"
fi
[ "$MODE" = check ] && { echo "--check なので何もしない。"; exit 0; }

if [ -n "$DIRTY" ] && [ "${DIRTY_OK:-}" != 1 ]; then
  echo "未コミットの変更がある。タグのハッシュと中身が食い違うので止める（DIRTY_OK=1 で続行）。" >&2
  exit 1
fi

echo; echo "== ビルド"
start=$(date +%s)
docker buildx build \
  --platform "$PLATFORM" \
  --load \
  --label "org.opencontainers.image.revision=$(git rev-parse HEAD)" \
  -f topcoat.Dockerfile \
  -t "$IMAGE:$REV" -t "$IMAGE:latest" \
  .
echo "build: $(( $(date +%s) - start ))s"
docker image inspect "$IMAGE:$REV" --format 'size: {{.Size}} bytes  arch: {{.Os}}/{{.Architecture}}'

echo; echo "== 起動確認（1 CPU・メモリ 256MB、使い捨ての COOKIE_KEY）"
NAME="topcoat-imagecheck-$$"
cleanup() { docker rm -f "$NAME" >/dev/null 2>&1 || true; }
trap cleanup EXIT
docker run -d --name "$NAME" --cpus 1 --memory 256m -p 127.0.0.1::80 \
  -e COOKIE_KEY="$(od -An -tx1 -N64 /dev/urandom | tr -d ' \n')" "$IMAGE:$REV" >/dev/null
PORT="$(docker port "$NAME" 80/tcp | head -1 | sed 's/.*://')"
ok=0
for _ in $(seq 1 20); do
  [ "$(curl -s -m 2 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/" || true)" = 200 ] && { ok=1; break; }
  sleep 0.5
done
[ "$ok" = 1 ] || { echo "起動確認に失敗した"; docker logs "$NAME" | tail -20; exit 1; }
for path in / /products /products/1 /cart /products/9999; do
  echo "  $path -> $(curl -s -m 5 -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT$path")"
done

[ "$MODE" = push ] || { echo; echo "push しない（--push で push する）。"; exit 0; }

if [ "${YES:-}" != 1 ]; then
  read -r -p "$IMAGE:$REV と :latest を push する。よいか [y/N] " ans
  [ "$ans" = y ] || { echo "中止"; exit 1; }
fi
echo; echo "== push"
docker push "$IMAGE:$REV"
docker push "$IMAGE:latest"
echo "pushed: $IMAGE:$REV"
