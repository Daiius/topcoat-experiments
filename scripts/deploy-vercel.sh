#!/usr/bin/env bash
# Deploys both storefronts to Vercel from the CLI and checks how they behave
# there. Procedure and background: docs/deploy-vercel.md.
#
#   topcoat -> Container Runtime (Beta), ./Dockerfile, build context = repo root
#   nextjs  -> native Next.js, Root Directory = nextjs/ (reads ../data at build)
#
# There is no Git remote (D9), so nothing is connected to Git: every deploy
# uploads the working tree from the repository root (minus .vercelignore).
# Both projects deploy from the same directory, so neither is `vercel link`ed
# there; the project is chosen per command with VERCEL_ORG_ID /
# VERCEL_PROJECT_ID, whose values setup records in deploy/projects.env.
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_DIR=$(cd -- "$SCRIPT_DIR/.." && pwd)
PROBE="$SCRIPT_DIR/vercel-probe.mjs"

STATE_DIR="$REPO_DIR/deploy"
PROJECTS_ENV="$STATE_DIR/projects.env"
URLS_ENV="$STATE_DIR/urls.env"

APPS=(topcoat nextjs)
TOPCOAT_PROJECT=${TOPCOAT_PROJECT:-topcoat-practice-topcoat}
NEXTJS_PROJECT=${NEXTJS_PROJECT:-topcoat-practice-nextjs}
NEXTJS_ROOT=nextjs
# copy-data also runs as `prebuild`; calling it explicitly does not depend on
# pnpm's pre/post script setting (same as nextjs/Dockerfile).
NEXTJS_BUILD_COMMAND='pnpm run copy-data && pnpm run build'

ROUTES=("/" "/products" "/products?q=headphones" "/products/1" "/cart" "/products/9999")
RUNS=${RUNS:-3}
HOLD_S=${HOLD_S:-330}
IMAGES_PER_APP=${IMAGES_PER_APP:-3}

ASSUME_YES=0
VERCEL=${VERCEL_BIN:-$(command -v vercel || true)}
[[ -n "$VERCEL" ]] || VERCEL="$HOME/.local/share/pnpm/bin/vercel"
SCOPE_ARGS=()
[[ -n "${VERCEL_SCOPE:-}" ]] && SCOPE_ARGS=(--scope "$VERCEL_SCOPE")

usage() {
    cat <<'EOF'
Usage: scripts/deploy-vercel.sh <command> [args] [--yes]

Commands:
  check                         Show CLI version, login, projects, settings and
                                env var names. Changes nothing.
  setup                         Create both projects, set their build settings
                                and register COOKIE_KEY (Production + Preview).
                                Idempotent: only missing pieces are applied.
  deploy [topcoat|nextjs|all] [--prod]
                                Deploy (default: all, preview). The first deploy
                                of a new project is production regardless.
                                Writes the URLs to deploy/urls.env.
  verify [topcoat|nextjs|all] [--quick]
                                Probe the deployed URLs; results go to
                                bench/results/vercel-<timestamp>/. Without
                                --quick it also holds one WebSocket (topcoat)
                                and one SSE stream (Next.js) open for HOLD_S
                                seconds (default 330) to see when they are cut.
  logs <topcoat|nextjs> [vercel logs options]
                                Request logs (add --follow for live runtime logs).
  rollback <topcoat|nextjs> [--undo]
                                Point production back at the previous production
                                deployment (Hobby: only the immediately previous
                                one). --undo promotes the newest one again.
  teardown [topcoat|nextjs|all] Delete the Vercel project(s) with all their
                                deployments and env vars, and the local state.

Options:
  -y, --yes    Do not ask; needed when stdin is not a terminal (e.g. `! cmd`
               in Claude Code), where the command otherwise only prints its plan.
  -h, --help   Show this help

Environment:
  TOPCOAT_PROJECT / NEXTJS_PROJECT  Project names (default topcoat-practice-topcoat /
                                    topcoat-practice-nextjs)
  VERCEL_SCOPE                      Team slug, if not the personal account
  VERCEL_BIN                        Path to the vercel CLI
  VERCEL_AUTOMATION_BYPASS_SECRET   Sent as x-vercel-protection-bypass by verify,
                                    for protected (preview) URLs
  RUNS (3), HOLD_S (330), IMAGES_PER_APP (3)   verify parameters

Local state (gitignored): deploy/projects.env (org/project IDs), deploy/urls.env.
Depends on: bash, curl, jq, node, vercel.
EOF
}

log() { printf '%s\n' "$*" >&2; }
step() { printf '\n== %s\n' "$*" >&2; }
die() { log "error: $*"; exit 1; }

vc() { "$VERCEL" "$@" --no-color "${SCOPE_ARGS[@]}"; }
# Vercel REST API through the CLI's login. Prints raw JSON; empty on failure.
vapi() { vc api "$@" --raw 2>/dev/null || true; }

need_tools() {
    local tool
    for tool in curl jq node; do
        command -v "$tool" >/dev/null || die "$tool is required"
    done
    [[ -x "$VERCEL" ]] || die "vercel CLI not found (set VERCEL_BIN)"
}

logged_in() { vc whoami >/dev/null 2>&1; }
need_login() { logged_in || die "not logged in: run 'vercel login' first"; }

# Asks before a change. Without a terminal, only --yes lets it through, so the
# plan printed above is all that happens on the first run.
confirm() {
    local prompt=$1 answer
    if ((ASSUME_YES)); then return 0; fi
    if [[ -t 0 ]]; then
        read -r -p "$prompt [y/N] " answer
        [[ "$answer" == [yY] || "$answer" == [yY][eE][sS] ]] && return 0
        log "aborted"
        exit 1
    fi
    log ""
    log "stdin is not a terminal: nothing was changed."
    log "If the plan above is right, run the same command again with --yes."
    exit 3
}

project_name() {
    case $1 in
        topcoat) printf '%s' "$TOPCOAT_PROJECT" ;;
        nextjs) printf '%s' "$NEXTJS_PROJECT" ;;
        *) die "unknown app: $1 (topcoat|nextjs)" ;;
    esac
}
upper() { printf '%s' "${1^^}"; }

# Project JSON from the API, or empty if it does not exist.
project_json() {
    local json
    json=$(vapi "/v9/projects/$(project_name "$1")")
    if [[ -n "$json" ]] && jq -e '.id' >/dev/null 2>&1 <<<"$json"; then
        printf '%s' "$json"
    fi
}

# KEY=value files under deploy/ (sourced by bash, so values are quoted).
state_set() {
    local file=$1 key=$2 value=$3
    mkdir -p "$STATE_DIR"
    touch "$file"
    local tmp
    tmp=$(mktemp "$STATE_DIR/.tmp.XXXXXX")
    grep -v "^${key}=" "$file" >"$tmp" || true
    printf '%s=%q\n' "$key" "$value" >>"$tmp"
    mv "$tmp" "$file"
}
state_unset_prefix() {
    local file=$1 prefix=$2
    [[ -f "$file" ]] || return 0
    local tmp
    tmp=$(mktemp "$STATE_DIR/.tmp.XXXXXX")
    grep -v "^${prefix}" "$file" >"$tmp" || true
    mv "$tmp" "$file"
}
load_state() {
    # shellcheck disable=SC1090
    [[ -f "$PROJECTS_ENV" ]] && source "$PROJECTS_ENV"
    # shellcheck disable=SC1090
    [[ -f "$URLS_ENV" ]] && source "$URLS_ENV"
    return 0
}
state_get() {
    local name=$1
    printf '%s' "${!name:-}"
}

# Runs a vercel command against one project, without a .vercel link.
vc_for() {
    local app=$1
    shift
    local up org project
    up=$(upper "$app")
    org=$(state_get VERCEL_ORG_ID)
    project=$(state_get "${up}_PROJECT_ID")
    [[ -n "$org" && -n "$project" ]] || die "$app: no IDs in deploy/projects.env (run setup)"
    (cd "$REPO_DIR" && VERCEL_ORG_ID="$org" VERCEL_PROJECT_ID="$project" vc "$@")
}

select_apps() {
    case ${1:-all} in
        all) SELECTED=("${APPS[@]}") ;;
        topcoat | nextjs) SELECTED=("$1") ;;
        *) die "unknown app: $1 (topcoat|nextjs|all)" ;;
    esac
}

# Names and targets of a project's env vars (values are never read).
env_targets() {
    local id=$1 key=$2
    vapi "/v10/projects/$id/env" |
        jq -r --arg key "$key" '[.envs[]? | select(.key == $key) | .target | if type == "array" then .[] else . end] | unique | join(",")' 2>/dev/null || true
}

# --- check ---------------------------------------------------------------------

cmd_check() {
    need_tools
    step "Tools"
    printf 'vercel  %s\n' "$("$VERCEL" --version 2>/dev/null | tail -n 1)"
    printf 'node    %s\n' "$(node --version)"
    printf 'curl    %s\n' "$(curl --version | head -n 1 | cut -d' ' -f2)"
    printf 'jq      %s\n' "$(jq --version)"

    step "Login"
    local user
    if user=$(vc whoami 2>/dev/null | tail -n 1) && [[ -n "$user" ]]; then
        printf 'logged in as %s%s\n' "$user" "${VERCEL_SCOPE:+ (scope $VERCEL_SCOPE)}"
    else
        printf 'NOT logged in -> run: vercel login\n'
        user=""
    fi

    step "Local state"
    printf '.vercelignore         %s\n' "$([[ -f "$REPO_DIR/.vercelignore" ]] && echo present || echo MISSING)"
    printf 'deploy/projects.env   %s\n' "$([[ -f "$PROJECTS_ENV" ]] && echo present || echo 'absent (setup writes it)')"
    [[ -f "$PROJECTS_ENV" ]] && sed 's/^/    /' "$PROJECTS_ENV"
    printf 'deploy/urls.env       %s\n' "$([[ -f "$URLS_ENV" ]] && echo present || echo 'absent (deploy writes it)')"
    [[ -f "$URLS_ENV" ]] && sed 's/^/    /' "$URLS_ENV"
    if [[ -f "$REPO_DIR/.vercel/project.json" ]]; then
        printf 'WARNING: .vercel/project.json exists at the repo root (a `vercel link`).\n'
        printf '         This script overrides it with VERCEL_PROJECT_ID, but plain `vercel` commands use it.\n'
    fi

    [[ -n "$user" ]] || return 0
    local app json id
    for app in "${APPS[@]}"; do
        step "Project: $app ($(project_name "$app"))"
        json=$(project_json "$app")
        if [[ -z "$json" ]]; then
            printf 'does not exist (setup creates it)\n'
            continue
        fi
        jq -r '
            "id                              \(.id)",
            "account (org) id                \(.accountId)",
            "framework                       \(.framework // "null")",
            "rootDirectory                   \(.rootDirectory // "null (repo root)")",
            "sourceFilesOutsideRootDirectory \(.sourceFilesOutsideRootDirectory | tostring)",
            "buildCommand                    \(.buildCommand // "null (framework default)")",
            "nodeVersion                     \(.nodeVersion // "null")",
            "fluid compute                   \(.resourceConfig.fluid | tostring)",
            "production URL                  \(.targets.production.alias // [] | join(" "))"
        ' <<<"$json"
        id=$(jq -r .id <<<"$json")
        printf 'env vars (name: targets)\n'
        vapi "/v10/projects/$id/env" |
            jq -r '.envs[]? | "    \(.key): \(.target | if type == "array" then join(",") else . end) (\(.type))"' 2>/dev/null || true
    done
}

# --- setup ---------------------------------------------------------------------

cmd_setup() {
    need_tools
    need_login
    load_state

    local tjson njson
    tjson=$(project_json topcoat)
    njson=$(project_json nextjs)

    local -a plan=()
    if [[ -z "$tjson" ]]; then
        plan+=("topcoat: create project $TOPCOAT_PROJECT"
            "topcoat: set framework = container"
            "topcoat: generate COOKIE_KEY (128 hex) and add it as a Sensitive env var for production,preview")
    else
        [[ "$(jq -r '.framework // ""' <<<"$tjson")" == container ]] || plan+=("topcoat: set framework = container")
        local targets
        targets=$(env_targets "$(jq -r .id <<<"$tjson")" COOKIE_KEY)
        [[ ",$targets," == *,production,* && ",$targets," == *,preview,* ]] ||
            plan+=("topcoat: add COOKIE_KEY for the missing targets (now: ${targets:-none})")
    fi
    if [[ -z "$njson" ]]; then
        plan+=("nextjs: create project $NEXTJS_PROJECT"
            "nextjs: set framework = nextjs, rootDirectory = $NEXTJS_ROOT, buildCommand = '$NEXTJS_BUILD_COMMAND'"
            "nextjs: set sourceFilesOutsideRootDirectory = true (the build copies ../data)"
            "nextjs: add ENABLE_EXPERIMENTAL_COREPACK=1 for production,preview (pnpm from packageManager)")
    else
        if [[ "$(jq -r '[.framework, .rootDirectory, .buildCommand] | map(. // "") | join("|")' <<<"$njson")" != "nextjs|$NEXTJS_ROOT|$NEXTJS_BUILD_COMMAND" ]]; then
            plan+=("nextjs: set framework = nextjs, rootDirectory = $NEXTJS_ROOT, buildCommand = '$NEXTJS_BUILD_COMMAND'")
        fi
        [[ "$(jq -r '.sourceFilesOutsideRootDirectory // false' <<<"$njson")" == true ]] ||
            plan+=("nextjs: set sourceFilesOutsideRootDirectory = true (the build copies ../data)")
        local ntargets
        ntargets=$(env_targets "$(jq -r .id <<<"$njson")" ENABLE_EXPERIMENTAL_COREPACK)
        [[ ",$ntargets," == *,production,* && ",$ntargets," == *,preview,* ]] ||
            plan+=("nextjs: add ENABLE_EXPERIMENTAL_COREPACK=1 for the missing targets (now: ${ntargets:-none})")
    fi
    if [[ -z "$tjson" || -z "$njson" || ! -f "$PROJECTS_ENV" ]]; then
        plan+=("local: write org/project IDs to deploy/projects.env")
    fi

    if ((${#plan[@]} == 0)); then
        log "Nothing to do: both projects are set up."
        return 0
    fi
    step "Plan"
    printf '  - %s\n' "${plan[@]}" >&2
    log ""
    log "Undo: scripts/deploy-vercel.sh teardown (deletes the projects and deploy/)."
    confirm "Apply this plan?"

    setup_topcoat
    setup_nextjs
    step "Done"
    sed 's/^/    /' "$PROJECTS_ENV" >&2
    log "Next: scripts/deploy-vercel.sh deploy all --prod"
}

ensure_project() {
    local app=$1 name json
    name=$(project_name "$app")
    json=$(project_json "$app")
    if [[ -z "$json" ]]; then
        step "$app: create project $name"
        vc project add "$name" >&2
        json=$(project_json "$app")
        [[ -n "$json" ]] || die "$app: project $name was not created"
    fi
    state_set "$PROJECTS_ENV" VERCEL_ORG_ID "$(jq -r .accountId <<<"$json")"
    state_set "$PROJECTS_ENV" "$(upper "$app")_PROJECT_ID" "$(jq -r .id <<<"$json")"
    state_set "$PROJECTS_ENV" "$(upper "$app")_PROJECT_NAME" "$name"
    printf '%s' "$json"
}

setup_topcoat() {
    local json id targets
    json=$(ensure_project topcoat)
    id=$(jq -r .id <<<"$json")
    if [[ "$(jq -r '.framework // ""' <<<"$json")" != container ]]; then
        step "topcoat: framework = container"
        vc project update "$TOPCOAT_PROJECT" --framework container --yes >&2
    fi
    targets=$(env_targets "$id" COOKIE_KEY)
    local missing=()
    [[ ",$targets," == *,production,* ]] || missing+=(production)
    [[ ",$targets," == *,preview,* ]] || missing+=(preview)
    if ((${#missing[@]})); then
        local joined
        joined=$(IFS=,; printf '%s' "${missing[*]}")
        step "topcoat: COOKIE_KEY -> $joined"
        # 64 random bytes = 128 hex digits. Passed on stdin, not argv, and
        # stored as Sensitive (cannot be read back; see docs/deploy-vercel.md).
        node -e 'process.stdout.write(require("node:crypto").randomBytes(64).toString("hex"))' |
            vc env add COOKIE_KEY "$joined" --project "$id" --sensitive --yes >&2
    fi
}

setup_nextjs() {
    local json id targets
    json=$(ensure_project nextjs)
    id=$(jq -r .id <<<"$json")
    if [[ "$(jq -r '[.framework, .rootDirectory, .buildCommand] | map(. // "") | join("|")' <<<"$json")" != "nextjs|$NEXTJS_ROOT|$NEXTJS_BUILD_COMMAND" ]]; then
        step "nextjs: framework / rootDirectory / buildCommand"
        vc project update "$NEXTJS_PROJECT" --framework nextjs --root-directory "$NEXTJS_ROOT" \
            --build-command "$NEXTJS_BUILD_COMMAND" --yes >&2
    fi
    if [[ "$(jq -r '.sourceFilesOutsideRootDirectory // false' <<<"$json")" != true ]]; then
        step "nextjs: sourceFilesOutsideRootDirectory = true"
        vc api "/v9/projects/$id" -X PATCH -F sourceFilesOutsideRootDirectory=true --silent >&2
    fi
    targets=$(env_targets "$id" ENABLE_EXPERIMENTAL_COREPACK)
    local missing=()
    [[ ",$targets," == *,production,* ]] || missing+=(production)
    [[ ",$targets," == *,preview,* ]] || missing+=(preview)
    if ((${#missing[@]})); then
        local joined
        joined=$(IFS=,; printf '%s' "${missing[*]}")
        step "nextjs: ENABLE_EXPERIMENTAL_COREPACK=1 -> $joined"
        vc env add ENABLE_EXPERIMENTAL_COREPACK "$joined" --project "$id" --value 1 --no-sensitive --yes >&2
    fi
}

# --- deploy --------------------------------------------------------------------

cmd_deploy() {
    local which=all prod=0 arg
    for arg in "$@"; do
        case $arg in
            --prod) prod=1 ;;
            topcoat | nextjs | all) which=$arg ;;
            *) die "deploy: unknown argument $arg" ;;
        esac
    done
    need_tools
    need_login
    load_state
    [[ -f "$REPO_DIR/.vercelignore" ]] || die ".vercelignore is missing: the upload would include topcoat/target"
    select_apps "$which"

    step "Plan"
    local app
    for app in "${SELECTED[@]}"; do
        log "  - $app -> project $(project_name "$app") ($( ((prod)) && echo production || echo 'preview; production if it is the first deploy'))"
    done
    log "  - upload: $REPO_DIR minus .vercelignore; the build runs on Vercel"
    log "  - writes: deploy/urls.env"
    log "Undo: scripts/deploy-vercel.sh rollback <app> (production), or teardown."
    confirm "Deploy?"

    for app in "${SELECTED[@]}"; do
        deploy_one "$app" "$prod"
    done
    step "URLs (deploy/urls.env)"
    sed 's/^/    /' "$URLS_ENV" >&2
    log "Next: scripts/deploy-vercel.sh verify"
}

deploy_one() {
    local app=$1 prod=$2 up url host json
    up=$(upper "$app")
    local -a args=(deploy --yes)
    ((prod)) && args+=(--prod)
    step "$app: vercel ${args[*]}"
    mkdir -p "$STATE_DIR"
    # stdout is the deployment URL; progress and build logs go to stderr.
    url=$(vc_for "$app" "${args[@]}" | tail -n 1)
    [[ "$url" == https://* ]] || die "$app: no deployment URL in the output ($url)"
    host=${url#https://}
    json=$(vapi "/v13/deployments/$host")
    local id target public
    id=$(jq -r '.id // empty' <<<"$json")
    target=$(jq -r '.target // "preview"' <<<"$json")
    # Standard Protection keeps deployment URLs private and leaves production
    # domains public, so verify uses the shortest *.vercel.app alias.
    public=$(jq -r --arg name "$(project_name "$app")" '
        [.alias[]? | select(endswith(".vercel.app"))] as $a
        | (($a | map(select(. == ($name + ".vercel.app")))) + ($a | sort_by(length)))[0] // empty' <<<"$json")
    if [[ "$target" == production && -z "$public" ]]; then
        public=$(project_json "$app" | jq -r '[.targets.production.alias[]? | select(endswith(".vercel.app"))] | sort_by(length) | .[0] // empty')
    fi
    state_set "$URLS_ENV" "${up}_DEPLOYMENT_URL" "$url"
    state_set "$URLS_ENV" "${up}_DEPLOYMENT_ID" "$id"
    state_set "$URLS_ENV" "${up}_TARGET" "$target"
    state_set "$URLS_ENV" "${up}_DEPLOYED_AT" "$(date -Iseconds)"
    if [[ "$target" == production && -n "$public" ]]; then
        state_set "$URLS_ENV" "${up}_URL" "https://$public"
    else
        state_set "$URLS_ENV" "${up}_URL" "$url"
    fi
}

# --- verify --------------------------------------------------------------------

cmd_verify() {
    local which=all quick=0 arg
    for arg in "$@"; do
        case $arg in
            --quick) quick=1 ;;
            topcoat | nextjs | all) which=$arg ;;
            *) die "verify: unknown argument $arg" ;;
        esac
    done
    need_tools
    load_state
    select_apps "$which"

    local out
    out="$REPO_DIR/bench/results/vercel-$(date +%Y%m%d-%H%M%S)"
    mkdir -p "$out/raw"
    log "results: ${out#"$REPO_DIR"/}"

    HDR=()
    [[ -n "${VERCEL_AUTOMATION_BYPASS_SECRET:-}" ]] &&
        HDR=(-H "x-vercel-protection-bypass: $VERCEL_AUTOMATION_BYPASS_SECRET")
    local -a probe_hdr=()
    [[ -n "${VERCEL_AUTOMATION_BYPASS_SECRET:-}" ]] &&
        probe_hdr=(--header "x-vercel-protection-bypass: $VERCEL_AUTOMATION_BYPASS_SECRET")

    local app base up status
    local -a waits=()
    : >"$out/meta.env"
    for app in "${SELECTED[@]}"; do
        up=$(upper "$app")
        base=$(state_get "${up}_URL")
        if [[ -z "$base" ]]; then
            log "$app: no ${up}_URL in deploy/urls.env (run deploy); skipped"
            continue
        fi
        printf '%s_URL=%s\n%s_DEPLOYMENT_URL=%s\n' "$up" "$base" "$up" "$(state_get "${up}_DEPLOYMENT_URL")" >>"$out/meta.env"
        status=$(curl -s -o /dev/null -w '%{http_code}' "${HDR[@]}" "$base/" || true)
        if [[ "$status" == 401 || "$status" == 403 ]]; then
            log "$app: $base/ answered $status: Deployment Protection."
            log "  Use the production URL (deploy --prod), or export VERCEL_AUTOMATION_BYPASS_SECRET."
            continue
        fi

        if ((!quick)); then
            step "$app: holding a live connection for ${HOLD_S}s in the background"
            if [[ $app == topcoat ]]; then
                node "$PROBE" ws "$base/products/1" --hold "$HOLD_S" "${probe_hdr[@]}" >"$out/$app-ws-hold.json" &
            else
                node "$PROBE" sse "$base/api/stock/1" --hold "$HOLD_S" "${probe_hdr[@]}" >"$out/$app-sse-hold.json" &
            fi
            waits+=($!)
        fi

        verify_routes "$app" "$base" "$out"
        verify_stream "$app" "$base" "$out"
        if [[ $app == topcoat ]]; then
            step "$app: WebSocket handshake on /products/1"
            local mode
            for mode in same origin-only none; do
                node "$PROBE" ws "$base/products/1" --origin "$mode" "${probe_hdr[@]}" | tee -a "$out/ws.jsonl" |
                    jq -r '"  origin=\(.origin_mode): status \(.status // "-") upgraded=\(.upgraded // false) \(.error // "")"'
            done
        else
            step "$app: SSE /api/stock/1 for 12s"
            node "$PROBE" sse "$base/api/stock/1" --hold 12 "${probe_hdr[@]}" | tee -a "$out/sse.jsonl" |
                jq -r '"  status \(.status // "-") ttfb \(.ttfb_ms // "-")ms, data events \(.data_events // 0), gaps \(.gaps_ms // [] | join(",")) \(.error // "")"'
        fi
        verify_images "$app" "$base" "$out"
    done

    if ((${#waits[@]})); then
        step "Waiting for the held connections (up to ${HOLD_S}s since start)"
        wait "${waits[@]}" || true
        local f
        for f in "$out"/*-hold.json; do
            [[ -f "$f" ]] || continue
            jq -r --arg f "$(basename "$f")" '"  \($f): status \(.status // "-"), " + (if .kind == "ws" then "open \(.open_ms // 0 | . / 1000 | floor)s, closed by \(.closed_by // "-") code \(.close_code // "-")" else "\(.data_events // 0) events, ended after \(.ended_after_ms // 0 | . / 1000 | floor)s by \(.ended_by // "-")" end)' "$f"
        done
    fi

    write_summary "$out"
    step "Summary: ${out#"$REPO_DIR"/}/summary.md"
    cat "$out/summary.md"
    cat >&2 <<'EOF'

Cold start is not measured here. Leave both apps without traffic for 6+ minutes
(production scales to zero after 5 minutes idle; preview after 30 seconds), then
measure the first request by hand, e.g.:
    curl -s -o /dev/null -w '%{http_code} ttfb=%{time_starttransfer}s\n' <URL>/products/2
Use a route the CDN has not cached (check x-vercel-cache: MISS) and record it in
the same results directory.
EOF
}

# One request: status, timings, size and the headers we care about, as JSON.
fetch_json() {
    local url=$1 raw=$2
    shift 2
    local w
    : >"$raw"
    w=$(curl -s -o /dev/null -D "$raw" --compressed -H 'accept-encoding: br, gzip' "${HDR[@]}" "$@" \
        -w '{"status":"%{http_code}","ttfb_s":%{time_starttransfer},"total_s":%{time_total},"bytes":%{size_download}}' "$url" || true)
    [[ -n "$w" ]] || w='{"status":"0"}'
    jq -c --rawfile h "$raw" '
        .status |= (tonumber? // 0)
        | . + {
            ttfb_ms: ((.ttfb_s // 0) * 1000 | . * 10 | round / 10),
            total_ms: ((.total_s // 0) * 1000 | . * 10 | round / 10),
            headers: ($h | split("\r\n") | map(select(test("^[A-Za-z0-9-]+:")) | capture("^(?<k>[^:]+):\\s*(?<v>.*)$") | {key: (.k | ascii_downcase), value: .v}) | from_entries
                | with_entries(select(.key | IN("cache-control", "x-vercel-cache", "age", "x-vercel-id", "content-type", "content-encoding", "x-nextjs-postponed", "x-nextjs-prerender", "x-matched-path"))))
        } | del(.ttfb_s, .total_s)' <<<"$w"
}

verify_routes() {
    local app=$1 base=$2 out=$3 route run i=0
    step "$app: routes x $RUNS (TTFB, Cache-Control, x-vercel-cache)"
    for route in "${ROUTES[@]}"; do
        i=$((i + 1))
        for run in $(seq 1 "$RUNS"); do
            fetch_json "$base$route" "$out/raw/$app-route$i-$run.headers" |
                jq -c --arg app "$app" --arg route "$route" --argjson run "$run" '{app: $app, route: $route, run: $run} + .' |
                tee -a "$out/routes.jsonl" |
                jq -r '"  \(.route) #\(.run): \(.status) ttfb \(.ttfb_ms)ms total \(.total_ms)ms cache=\(.headers["x-vercel-cache"] // "-") cc=\(.headers["cache-control"] // "-")"'
        done
    done
}

verify_stream() {
    local app=$1 base=$2 out=$3 enc run
    step "$app: chunk arrival on /products/1"
    for enc in identity br; do
        for run in 1 2; do
            node "$PROBE" stream "$base/products/1" --encoding "$enc" "${probe_hdr[@]}" |
                jq -c --arg app "$app" --argjson run "$run" '{app: $app, run: $run} + .' |
                tee -a "$out/stream.jsonl" |
                jq -r '"  \(.encoding) #\(.run): \(.status // "-") ttfb \(.ttfb_ms // "-")ms, bursts at " + ([.bursts[]? | "\(.start_ms)ms(\(.bytes)B)"] | join(" ")) + " cache=\(.headers["x-vercel-cache"] // "-") \(.error // "")"'
        done
    done
}

verify_images() {
    local app=$1 base=$2 out=$3 html urls u run i=0
    step "$app: images from /products (x-vercel-cache on 2 requests each)"
    html=$(curl -s --compressed "${HDR[@]}" "$base/products" || true)
    # grep finds nothing -> status 1; an empty list is handled below.
    if [[ $app == topcoat ]]; then
        # Every .webp in src and srcset (the asset bundle's hashed URLs).
        urls=$(grep -oE '[^" ,]+\.webp' <<<"$html" | sort -u | head -n "$IMAGES_PER_APP" || true)
    else
        urls=$(grep -oE 'src="/_next/image\?[^"]+"' <<<"$html" | sed -E 's/^src="//; s/"$//; s/&amp;/\&/g' | sort -u | head -n "$IMAGES_PER_APP" || true)
    fi
    if [[ -z "$urls" ]]; then
        log "  no image URLs found"
        return 0
    fi
    while IFS= read -r u; do
        i=$((i + 1))
        [[ "$u" == http* ]] || u="$base$u"
        for run in 1 2; do
            fetch_json "$u" "$out/raw/$app-image$i-$run.headers" -H 'accept: image/avif,image/webp,*/*' |
                jq -c --arg app "$app" --arg url "$u" --argjson run "$run" '{app: $app, url: $url, run: $run} + .' |
                tee -a "$out/images.jsonl" |
                jq -r '"  #\(.run) \(.url | sub("^https://[^/]+"; "")): \(.status) \(.bytes)B \(.headers["content-type"] // "-") cache=\(.headers["x-vercel-cache"] // "-") ttfb \(.ttfb_ms)ms"'
        done
    done <<<"$urls"
}

write_summary() {
    local out=$1
    {
        printf '# Vercel verify %s\n\n' "$(basename "$out")"
        sed 's/^/- /' "$out/meta.env"
        printf '\n## Routes (TTFB ms per run; x-vercel-cache per run)\n\n'
        printf '| app | route | status | TTFB | x-vercel-cache | Cache-Control |\n|---|---|---|---|---|---|\n'
        [[ -f "$out/routes.jsonl" ]] && jq -s -r '
            group_by(.app + " " + .route)[] as $g
            | $g[0] as $f
            | "| \($f.app) | `\($f.route)` | \([$g[].status] | unique | join(",")) | \([$g[].ttfb_ms] | join(" / ")) | \([$g[].headers["x-vercel-cache"] // "-"] | join(" / ")) | `\($f.headers["cache-control"] // "-")` |"' "$out/routes.jsonl"
        printf '\n## Streaming /products/1 (burst start ms)\n\n'
        printf '| app | encoding | run | TTFB | bursts (ms: bytes) |\n|---|---|---|---|---|\n'
        [[ -f "$out/stream.jsonl" ]] && jq -r '"| \(.app) | \(.encoding) | \(.run) | \(.ttfb_ms // "-") | \([.bursts[]? | "\(.start_ms): \(.bytes)"] | join(", ")) |"' "$out/stream.jsonl"
        printf '\n## Live connections\n\n'
        [[ -f "$out/ws.jsonl" ]] && jq -r '"- topcoat WS origin=\(.origin_mode): status \(.status // "-"), upgraded \(.upgraded // false)"' "$out/ws.jsonl"
        [[ -f "$out/sse.jsonl" ]] && jq -r '"- nextjs SSE 12s: status \(.status // "-"), \(.data_events // 0) events, gaps \(.gaps_ms // [] | join(","))ms"' "$out/sse.jsonl"
        local f
        for f in "$out"/*-hold.json; do
            [[ -f "$f" ]] || continue
            jq -r '"- held \(.kind): " + (if .kind == "ws" then "open \(.open_ms // 0 | . / 1000 | floor)s, closed by \(.closed_by // "-") (code \(.close_code // "-"))" else "\(.data_events // 0) events, ended after \(.ended_after_ms // 0 | . / 1000 | floor)s by \(.ended_by // "-")" end)' "$f"
        done
        printf '\n## Images\n\n| app | url | run | bytes | type | x-vercel-cache |\n|---|---|---|---|---|---|\n'
        [[ -f "$out/images.jsonl" ]] && jq -r '"| \(.app) | `\(.url | sub("^https://[^/]+"; ""))` | \(.run) | \(.bytes) | \(.headers["content-type"] // "-") | \(.headers["x-vercel-cache"] // "-") |"' "$out/images.jsonl"
        printf '\nCold start: not measured (see docs/deploy-vercel.md).\n'
    } >"$out/summary.md"
}

# --- logs / rollback / teardown -------------------------------------------------

cmd_logs() {
    local app=${1:-}
    [[ $app == topcoat || $app == nextjs ]] || die "logs: topcoat|nextjs"
    shift
    need_login
    load_state
    vc_for "$app" logs "$@"
}

# Recent production deployments, newest first: "url<TAB>state<TAB>created".
production_deployments() {
    local id=$1
    vapi "/v6/deployments?projectId=$id&target=production&limit=5" |
        jq -r '.deployments[]? | "https://\(.url)\t\(.state // .readyState)\t\(.created / 1000 | todate)"' 2>/dev/null || true
}

cmd_rollback() {
    local app=${1:-} undo=0 arg
    [[ $app == topcoat || $app == nextjs ]] || die "rollback: topcoat|nextjs [--undo]"
    shift
    for arg in "$@"; do
        case $arg in
            --undo) undo=1 ;;
            *) die "rollback: unknown argument $arg" ;;
        esac
    done
    need_tools
    need_login
    load_state
    local id list
    id=$(state_get "$(upper "$app")_PROJECT_ID")
    [[ -n "$id" ]] || die "$app: no project ID (run setup)"
    list=$(production_deployments "$id")
    step "$app: recent production deployments (newest first)"
    [[ -n "$list" ]] && sed 's/^/  /' <<<"$list" >&2
    local newest previous
    newest=$(sed -n 1p <<<"$list" | cut -f1)
    previous=$(sed -n 2p <<<"$list" | cut -f1)

    if ((undo)); then
        [[ -n "$newest" ]] || die "no production deployment"
        step "Plan"
        log "  - vercel promote $newest"
        log "    (production domains point at it again and auto-assignment is re-enabled)"
        confirm "Promote?"
        vc_for "$app" promote "$newest" --yes
        return 0
    fi

    [[ -n "$previous" ]] || die "no previous production deployment to roll back to"
    step "Plan"
    log "  - vercel rollback $previous"
    log "  Hobby can roll back only to the immediately previous production deployment."
    log "  Afterwards new production deploys are NOT auto-assigned to the domain;"
    log "  undo with: scripts/deploy-vercel.sh rollback $app --undo"
    confirm "Roll back?"
    vc_for "$app" rollback "$previous" --yes
}

cmd_teardown() {
    local which=${1:-all}
    need_tools
    need_login
    load_state
    select_apps "$which"
    step "Plan"
    local app id
    for app in "${SELECTED[@]}"; do
        id=$(state_get "$(upper "$app")_PROJECT_ID")
        if [[ -z "$id" ]]; then
            id=$(project_json "$app" | jq -r '.id // empty')
        fi
        if [[ -n "$id" ]]; then
            log "  - DELETE project $(project_name "$app") ($id): all deployments, env vars (COOKIE_KEY is lost), domains"
        else
            log "  - $app: no project found, nothing to delete on Vercel"
        fi
        log "  - remove ${app^^}_* from deploy/projects.env and deploy/urls.env"
    done
    log "This cannot be undone. To stop traffic but keep everything, use instead:"
    log "  vercel project pause <name>   (resume: vercel project resume <name>)"
    confirm "Delete?"
    for app in "${SELECTED[@]}"; do
        id=$(state_get "$(upper "$app")_PROJECT_ID")
        [[ -n "$id" ]] || id=$(project_json "$app" | jq -r '.id // empty')
        if [[ -n "$id" ]]; then
            step "$app: delete project $id"
            vc api "/v9/projects/$id" -X DELETE --dangerously-skip-permissions --silent >&2
        fi
        state_unset_prefix "$PROJECTS_ENV" "${app^^}_"
        state_unset_prefix "$URLS_ENV" "${app^^}_"
    done
}

# --- main ----------------------------------------------------------------------

main() {
    local -a args=()
    local arg
    for arg in "$@"; do
        case $arg in
            -y | --yes) ASSUME_YES=1 ;;
            -h | --help) usage; exit 0 ;;
            *) args+=("$arg") ;;
        esac
    done
    ((${#args[@]})) || { usage; exit 1; }
    local command=${args[0]}
    local -a rest=("${args[@]:1}")
    case $command in
        check) cmd_check ;;
        setup) cmd_setup ;;
        deploy) cmd_deploy "${rest[@]}" ;;
        verify) cmd_verify "${rest[@]}" ;;
        logs) cmd_logs "${rest[@]}" ;;
        rollback) cmd_rollback "${rest[@]}" ;;
        teardown) cmd_teardown "${rest[@]}" ;;
        help) usage ;;
        *) usage; exit 1 ;;
    esac
}

main "$@"
