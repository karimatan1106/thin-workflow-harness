#!/usr/bin/env bash
# PreToolUse(Read) guard: 大きいコードファイルの「全文 Read」を抑制し、
# LSP シンボル検索（Serena / harness-lspd=CKG）→ 必要箇所だけ offset/limit 付き Read に誘導する。
#
# 通す条件（いずれかで exit 0）:
#   - offset か limit がある（= ranged read、既に良い）
#   - コード拡張子でない（.md/.json/.toml/.png/設定などは対象外）
#   - ファイルが小さい（<= THRESHOLD_LINES 行）か、存在しない/読めない
# 作動条件: コード拡張子 かつ offset/limit 無し かつ THRESHOLD_LINES 行超 → exit 2 でブロック。
#   逃げ道: limit を付ければ意図的な全文 Read として通る。
#
# しきい値は環境変数 READ_LSP_THRESHOLD で上書き可（既定 400 行）。
# 無効化したいときは READ_LSP_GUARD_DISABLED=1。

[ -n "${READ_LSP_GUARD_DISABLED:-}" ] && exit 0
command -v jq >/dev/null 2>&1 || exit 0   # jq 無しでは判定不能 → 素通り

THRESHOLD="${READ_LSP_THRESHOLD:-400}"

INPUT=$(cat)
FILE=$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // empty')
OFFSET=$(printf '%s' "$INPUT" | jq -r '.tool_input.offset // empty')
LIMIT=$(printf '%s' "$INPUT" | jq -r '.tool_input.limit // empty')

# ranged read は常に許可
[ -n "$OFFSET" ] && exit 0
[ -n "$LIMIT" ] && exit 0
[ -z "$FILE" ] && exit 0

# 拡張子（小文字化）を取り出す
ext="${FILE##*.}"
ext=$(printf '%s' "$ext" | tr '[:upper:]' '[:lower:]')
case "$ext" in
  rs|ts|tsx|js|jsx|mjs|cjs|py|go|java|rb|php|c|cc|cpp|cxx|h|hpp|cs|swift|kt|scala) ;;  # コードのみ対象
  *) exit 0 ;;
esac

# Windows パス（バックスラッシュ）を bash 用に正規化
fpath=$(printf '%s' "$FILE" | tr '\\' '/')
[ -f "$fpath" ] || exit 0

LINES=$(wc -l < "$fpath" 2>/dev/null | tr -d ' ')
[ -z "$LINES" ] && exit 0
[ "$LINES" -le "$THRESHOLD" ] 2>/dev/null && exit 0

# ここまで来たら「大きいコードファイルの全文 Read」→ ブロック
{
  echo "[read-lsp-guard] 大きいコードファイル(${LINES} 行 > ${THRESHOLD})の全文 Read はコンテキストを圧迫します。"
  echo "まず LSP シンボル検索で位置特定 → 必要箇所だけ offset/limit 付きで Read してください:"
  echo "  - Serena MCP: mcp__serena__get_symbols_overview / find_symbol / find_referencing_symbols"
  echo "  - harness リポ内: harness-lspd query symbol|refs|closure|impacted-by"
  echo "全文が本当に必要なら limit を明示して意図的に Read してください(その場合は通ります)。"
} >&2
exit 2
