#!/usr/bin/env bash
# Side-by-side measurement of the topcoat and Next.js storefronts running in
# docker (bench/compose.yml). See bench/README.md for the conditions.
set -euo pipefail

BENCH_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
REPO_DIR=$(cd -- "$BENCH_DIR/.." && pwd)
COMPOSE_FILE="$BENCH_DIR/compose.yml"

APPS=(topcoat nextjs)
declare -A PORT=([topcoat]=8081 [nextjs]=8082)
declare -A CONTAINER=([topcoat]=bench-topcoat [nextjs]=bench-nextjs)
declare -A IMAGE=([topcoat]=storefront-topcoat:bench [nextjs]=storefront-nextjs:bench)

# Measured routes. /products/9999 is an id that does not exist.
ROUTES=("/" "/products" "/products?q=headphones" "/products/1" "/cart" "/products/9999")
# Pages whose subresources `assets` totals.
ASSET_PAGES=("/" "/products" "/products/1" "/cart")
STREAM_ROUTE="/products/1"
HEALTH_PATH="/"

# Defaults; overridable by flags (or the same-named environment variables).
COLD_RUNS=${COLD_RUNS:-5}
ROUTE_RUNS=${ROUTE_RUNS:-5}
STREAM_RUNS=${STREAM_RUNS:-3}
DURATION=${DURATION:-20s}
CONCURRENCY=${CONCURRENCY:-32}
RATE=${RATE:-200}
HEALTH_TIMEOUT=${HEALTH_TIMEOUT:-60}
STOP_TIMEOUT=${STOP_TIMEOUT:-10}
BURST_GAP_MS=${BURST_GAP_MS:-50}
VIEWPORT=${VIEWPORT:-1280}
DPR=${DPR:-1}
OUT=${RESULTS_DIR:-}
SELECTED_APPS=("${APPS[@]}")

usage() {
    cat <<'EOF'
Usage: bench/run.sh <command> [options]

Commands (each writes into bench/results/<timestamp>/ unless -o is given):
  images     Image sizes (docker image inspect .Size)              -> images.json
  coldstart  docker start -> first 200 on /, N times per app        -> coldstart.json
  routes     TTFB / total / size (identity, br, gzip), median of N  -> routes.json
  stream     Arrival times of the chunks of /products/1             -> stream.json
  load       oha: fixed duration x concurrency, and a fixed rate;
             memory (docker stats) at idle and after the load       -> load.json, memory.json
  assets     JS / CSS / image bytes each page references            -> assets.json
  all        images, coldstart, routes, stream, assets, load, then summary
  summary    Rebuild summary.md from the JSON in a results dir (-o DIR)

Options:
  -o, --out DIR           Results directory (default: bench/results/<timestamp>)
  -a, --apps "A B"        Apps to measure (default: "topcoat nextjs")
  -n, --runs N            Runs for coldstart and routes (default: 5)
  -d, --duration D        oha duration per run (default: 20s)
  -c, --concurrency N     oha concurrency (default: 32)
  -r, --rate N            oha fixed rate in requests/s (default: 200)
  -h, --help              Show this help

Environment (defaults): STREAM_RUNS (3), HEALTH_TIMEOUT s (60), STOP_TIMEOUT s (10),
  BURST_GAP_MS (50), VIEWPORT px (1280) and DPR (1) for picking srcset candidates,
  RESULTS_DIR (same as -o).

Prerequisites: images built and COOKIE_KEY exported (see bench/compose.yml).
Containers are created on demand with `docker compose up -d --no-build`.
Only the app being measured runs; the other is stopped.
Depends on: bash, curl, jq, oha, docker (with compose).
EOF
}

log() { printf '[%(%H:%M:%S)T] %s\n' -1 "$*" >&2; }
die() { log "error: $*"; exit 1; }

now_ns() { date +%s%N; }
# Milliseconds between two now_ns values, one decimal.
ms_between() { awk -v a="$1" -v b="$2" 'BEGIN { printf "%.1f", (b - a) / 1e6 }'; }

url_of() { printf 'http://127.0.0.1:%s%s' "${PORT[$1]}" "$2"; }

require() {
    local tool
    for tool in curl jq docker "$@"; do
        command -v "$tool" >/dev/null || die "$tool is not installed"
    done
}

init_out() {
    if [[ -z $OUT ]]; then
        OUT="$BENCH_DIR/results/$(date +%Y%m%d-%H%M%S)"
    fi
    mkdir -p "$OUT"
    if [[ ! -f $OUT/meta.json ]]; then
        write_meta
    fi
    log "results: $OUT"
}

write_meta() {
    local cpu
    cpu=$(awk -F': ' '/^model name/ { print $2; exit }' /proc/cpuinfo 2>/dev/null || true)
    jq -n \
        --arg date "$(date -Iseconds)" \
        --arg kernel "$(uname -r)" \
        --arg cpu "$cpu" \
        --arg nproc "$(nproc)" \
        --arg mem "$(awk '/^MemTotal/ { print $2 * 1024 }' /proc/meminfo)" \
        --arg docker "$(docker version --format '{{.Server.Version}}' 2>/dev/null || true)" \
        --arg curl "$(curl --version | head -n1)" \
        --arg oha "$(oha --version 2>/dev/null || true)" \
        --arg commit "$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || true)" \
        --arg dirty "$(git -C "$REPO_DIR" status --porcelain 2>/dev/null | head -c1)" \
        --arg duration "$DURATION" --arg concurrency "$CONCURRENCY" --arg rate "$RATE" \
        --arg cold_runs "$COLD_RUNS" --arg route_runs "$ROUTE_RUNS" \
        '{date: $date, host: {kernel: $kernel, cpu: $cpu, nproc: ($nproc | tonumber),
          mem_bytes: ($mem | tonumber)},
          tools: {docker: $docker, curl: $curl, oha: $oha},
          commit: $commit, dirty: ($dirty != ""),
          params: {duration: $duration, concurrency: ($concurrency | tonumber),
                   rate: ($rate | tonumber), cold_runs: ($cold_runs | tonumber),
                   route_runs: ($route_runs | tonumber)}}' >"$OUT/meta.json"
}

# --- container control --------------------------------------------------------

container_exists() { docker container inspect "$1" >/dev/null 2>&1; }
container_running() {
    [[ $(docker container inspect -f '{{.State.Running}}' "$1" 2>/dev/null || echo false) == true ]]
}

# Polls the health path until it answers 200. Fails early if the container
# exits (e.g. COOKIE_KEY missing) and after HEALTH_TIMEOUT seconds.
wait_ready() {
    local app=$1 url deadline code
    url=$(url_of "$app" "$HEALTH_PATH")
    deadline=$(($(date +%s) + HEALTH_TIMEOUT))
    while :; do
        code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 "$url" || true)
        [[ $code == 200 ]] && return 0
        if ! container_running "${CONTAINER[$app]}"; then
            docker logs --tail 20 "${CONTAINER[$app]}" >&2 || true
            die "${CONTAINER[$app]} is not running"
        fi
        (($(date +%s) < deadline)) || die "$url did not answer 200 within ${HEALTH_TIMEOUT}s (last: $code)"
        sleep 0.01 # poll interval
    done
}

start_app() {
    local app=$1 c=${CONTAINER[$1]}
    if ! container_exists "$c"; then
        docker compose -f "$COMPOSE_FILE" up -d --no-build "$app" >&2
    elif ! container_running "$c"; then
        docker start "$c" >/dev/null
    fi
    wait_ready "$app"
}

stop_app() {
    local c=${CONTAINER[$1]}
    if container_running "$c"; then
        docker stop -t "$STOP_TIMEOUT" "$c" >/dev/null
    fi
}

# Runs only $1: stops every other app, then starts $1 and waits until ready.
only_app() {
    local app=$1 other
    for other in "${APPS[@]}"; do
        [[ $other == "$app" ]] || stop_app "$other"
    done
    start_app "$app"
}

# One request per route so that lazily loaded code is not measured as a sample.
warm_up() {
    local app=$1 route
    for route in "${ROUTES[@]}"; do
        curl -s -o /dev/null "$(url_of "$app" "$route")" || true
    done
}

# Memory usage as docker stats reports it (cgroup usage minus inactive file
# cache), in bytes.
mem_usage_bytes() {
    docker stats --no-stream --format '{{.MemUsage}}' "$1" | awk '{
        v = $1; unit = v; sub(/^[0-9.]+/, "", unit); sub(/[A-Za-z]+$/, "", v)
        m = 1
        if (unit == "KiB" || unit == "kB") m = 1024
        else if (unit == "MiB" || unit == "MB") m = 1024 ^ 2
        else if (unit == "GiB" || unit == "GB") m = 1024 ^ 3
        printf "%d\n", v * m
    }'
}

# curl -w template: all transfer info as one JSON line, then the headers.
CURL_W='%{json}\n%{header_json}\n'

# Prints one JSON object for a request: code, ttfb_ms, total_ms, bytes,
# content_encoding, cache_control.
fetch_json() {
    local url=$1 encoding=$2
    curl -s -o /dev/null -H "Accept-Encoding: $encoding" -w "$CURL_W" "$url" | jq -sc '
        .[0] as $t | .[1] as $h
        | {code: $t.http_code,
           ttfb_ms: ($t.time_starttransfer * 1000),
           total_ms: ($t.time_total * 1000),
           bytes: $t.size_download,
           content_encoding: ($h["content-encoding"][0] // null),
           cache_control: ($h["cache-control"][0] // null)}'
}

JQ_LIB='
def median: sort | if length == 0 then null
    elif length % 2 == 1 then .[(length - 1) / 2]
    else (.[length / 2 - 1] + .[length / 2]) / 2 end;
def r1: if . == null then null else (. * 10 | round) / 10 end;
'

# --- commands -----------------------------------------------------------------

cmd_images() {
    local app size
    for app in "${SELECTED_APPS[@]}"; do
        size=$(docker image inspect -f '{{.Size}}' "${IMAGE[$app]}") ||
            die "image ${IMAGE[$app]} not found; build it first"
        jq -nc --arg app "$app" --arg image "${IMAGE[$app]}" --argjson size "$size" \
            '{app: $app, image: $image, size_bytes: $size}'
    done | jq -s . >"$OUT/images.json"
    log "images: $(jq -c 'map({(.app): .size_bytes}) | add' "$OUT/images.json")"
}

cmd_coldstart() {
    local app c i t0 t1 t2 t3 detail raw="$OUT/coldstart.jsonl"
    : >"$raw"
    for app in "${SELECTED_APPS[@]}"; do
        c=${CONTAINER[$app]}
        only_app "$app"
        for ((i = 1; i <= COLD_RUNS; i++)); do
            t0=$(now_ns)
            docker stop -t "$STOP_TIMEOUT" "$c" >/dev/null
            t1=$(now_ns)
            docker start "$c" >/dev/null
            t2=$(now_ns)
            wait_ready "$app"
            t3=$(now_ns)
            # The first request to a route that was not touched yet.
            detail=$(fetch_json "$(url_of "$app" "$STREAM_ROUTE")" identity)
            jq -nc --arg app "$app" --argjson run "$i" \
                --argjson stop "$(ms_between "$t0" "$t1")" \
                --argjson start_cmd "$(ms_between "$t1" "$t2")" \
                --argjson first_200 "$(ms_between "$t1" "$t3")" \
                --argjson detail "$detail" \
                '{app: $app, run: $run, stop_ms: $stop, docker_start_ms: $start_cmd,
                  first_200_ms: $first_200, first_detail: $detail}' | tee -a "$raw" >&2
        done
    done
    jq -s "$JQ_LIB"'
        group_by(.app) | map({
            app: .[0].app, runs: length,
            first_200_ms: {median: (map(.first_200_ms) | median | r1), min: (map(.first_200_ms) | min),
                           max: (map(.first_200_ms) | max)},
            docker_start_ms: (map(.docker_start_ms) | median | r1),
            stop_ms: (map(.stop_ms) | median | r1),
            first_detail_ttfb_ms: (map(.first_detail.ttfb_ms) | median | r1)})' \
        "$raw" >"$OUT/coldstart.json"
}

cmd_routes() {
    local app route enc i sample raw="$OUT/routes.jsonl"
    : >"$raw"
    for app in "${SELECTED_APPS[@]}"; do
        only_app "$app"
        warm_up "$app"
        for route in "${ROUTES[@]}"; do
            for enc in identity br gzip; do
                for ((i = 1; i <= ROUTE_RUNS; i++)); do
                    sample=$(fetch_json "$(url_of "$app" "$route")" "$enc")
                    jq -c --arg app "$app" --arg route "$route" --arg enc "$enc" --argjson run "$i" \
                        '{app: $app, route: $route, enc: $enc, run: $run} + .' <<<"$sample" >>"$raw"
                done
            done
            log "routes: $app $route"
        done
    done
    jq -s "$JQ_LIB"'
        group_by(.app + " " + .route) | map(
            . as $g
            | ($g | map(select(.enc == "identity"))) as $id
            | {app: $g[0].app, route: $g[0].route,
               code: ($id | map(.code) | unique),
               ttfb_ms: ($id | map(.ttfb_ms) | median | r1),
               total_ms: ($id | map(.total_ms) | median | r1),
               bytes: ($id | map(.bytes) | median),
               cache_control: $id[0].cache_control,
               br: ($g | map(select(.enc == "br")) | {bytes: (map(.bytes) | median),
                     content_encoding: .[0].content_encoding}),
               gzip: ($g | map(select(.enc == "gzip")) | {bytes: (map(.bytes) | median),
                     content_encoding: .[0].content_encoding})})' \
        "$raw" >"$OUT/routes.json"
}

# Turns a `curl --trace-ascii --trace-time` log into "t_ms bytes" lines, one
# per received data block, relative to the moment the request was sent.
trace_to_chunks() {
    awk '
        function sec(ts,  a) { split(ts, a, ":"); return a[1] * 3600 + a[2] * 60 + a[3] }
        $2 == "=>" && $3 == "Send" && $4 == "header," && t0 == "" { t0 = sec($1) }
        $2 == "<=" && $3 == "Recv" && $4 == "data," && t0 != "" {
            printf "%.1f %d\n", (sec($1) - t0) * 1000, $5
        }' "$1"
}

cmd_stream() {
    local app enc i trace url meta raw="$OUT/stream.jsonl"
    trace=$(mktemp)
    : >"$raw"
    for app in "${SELECTED_APPS[@]}"; do
        only_app "$app"
        warm_up "$app"
        url=$(url_of "$app" "$STREAM_ROUTE")
        for enc in identity "br, gzip"; do
            for ((i = 1; i <= STREAM_RUNS; i++)); do
                meta=$(curl -s -o /dev/null -H "Accept-Encoding: $enc" \
                    --trace-ascii "$trace" --trace-time -w '%{json}' "$url")
                trace_to_chunks "$trace" | jq -Rn --arg app "$app" --arg enc "$enc" \
                    --argjson run "$i" --argjson gap "$BURST_GAP_MS" --argjson t "$meta" '
                    [inputs | split(" ") | {t_ms: (.[0] | tonumber), bytes: (.[1] | tonumber)}] as $c
                    | {app: $app, enc: $enc, run: $run,
                       ttfb_ms: ($t.time_starttransfer * 1000), total_ms: ($t.time_total * 1000),
                       first_data_ms: ($c[0].t_ms // null), last_data_ms: ($c[-1].t_ms // null),
                       bytes: ($c | map(.bytes) | add), chunks: $c,
                       bursts: (reduce $c[] as $x ([];
                           if length > 0 and ($x.t_ms - .[-1].end_ms) <= $gap
                           then .[-1].end_ms = $x.t_ms | .[-1].bytes += $x.bytes
                           else . + [{start_ms: $x.t_ms, end_ms: $x.t_ms, bytes: $x.bytes}] end))}' \
                    >>"$raw"
            done
        done
        log "stream: $app"
    done
    rm -f "$trace"
    jq -s "$JQ_LIB"'
        group_by(.app + " " + .enc) | map({
            app: .[0].app, enc: .[0].enc, runs: length,
            first_data_ms: (map(.first_data_ms) | median | r1),
            last_data_ms: (map(.last_data_ms) | median | r1),
            bytes: (map(.bytes) | median),
            bursts_run1: (.[0].bursts | map({start_ms: (.start_ms | r1), bytes}))})' \
        "$raw" >"$OUT/stream.json"
}

# Prints "type<TAB>lazy<TAB>url" for every subresource an HTML page references:
# <script src>, <link rel=stylesheet|preload|modulepreload href>, <img src|srcset>.
# For <img srcset sizes>, picks the candidate a browser with a VIEWPORT-wide
# window at DPR would (sizes evaluated for min-width / max-width in px).
extract_refs() {
    tr '\n' ' ' | grep -oiE '<(script|link|img)[[:space:]][^>]*>' | awk -v vw="$VIEWPORT" -v dpr="$DPR" '
        # Attribute names are matched case-insensitively (React writes srcSet,
        # imageSrcSet, imageSizes); values keep their case.
        function attr(tag, name,  re, s) {
            re = "[[:space:]]" name "=\"[^\"]*\""
            if (!match(tolower(tag), re)) return ""
            s = substr(tag, RSTART, RLENGTH)
            sub(/^[[:space:]][^=]*="/, "", s); sub(/"$/, "", s)
            gsub(/&amp;/, "\\&", s)
            return s
        }
        function slot(sizes,  n, parts, i, p, cond, len, v) {
            n = split(sizes, parts, ",")
            for (i = 1; i <= n; i++) {
                p = parts[i]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", p)
                cond = ""
                if (match(p, /^\([^)]*\)/)) {
                    cond = substr(p, 2, RLENGTH - 2); p = substr(p, RLENGTH + 1)
                    gsub(/^[[:space:]]+/, "", p)
                }
                if (cond != "") {
                    v = cond; gsub(/[^0-9.]/, "", v)
                    if (cond ~ /min-width/ && vw < v + 0) continue
                    if (cond ~ /max-width/ && vw > v + 0) continue
                }
                len = p; gsub(/[^0-9.]/, "", len)
                if (p ~ /vw$/) return vw * len / 100
                return len + 0
            }
            return vw
        }
        function pick(srcset, sizes,  n, cands, i, c, w, best, bestw, maxw, maxu, need) {
            need = slot(sizes) * dpr; best = ""; bestw = 0; maxw = 0
            n = split(srcset, cands, ",")
            for (i = 1; i <= n; i++) {
                c = cands[i]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", c)
                split(c, f, /[[:space:]]+/)
                w = f[2]; sub(/w$/, "", w); w += 0
                if (w > maxw) { maxw = w; maxu = f[1] }
                if (w >= need && (best == "" || w < bestw)) { best = f[1]; bestw = w }
            }
            return best != "" ? best : maxu
        }
        {
            tag = $0; name = tolower(substr(tag, 2, 4)); u = ""; type = ""
            lazy = (tolower(tag) ~ /loading="lazy"/) ? "lazy" : "eager"
            rel = tolower(attr(tag, "rel")); as = tolower(attr(tag, "as"))
            if (name == "scri") { u = attr(tag, "src"); type = "js" }
            else if (name == "link") {
                u = attr(tag, "href")
                if (rel == "stylesheet" || as == "style") type = "css"
                else if (rel == "modulepreload" || as == "script") type = "js"
                else if (as == "image") {
                    type = "img"
                    if (attr(tag, "imagesrcset") != "")
                        u = pick(attr(tag, "imagesrcset"), attr(tag, "imagesizes"))
                }
                else if (as == "font") type = "font"
            } else if (name == "img ") {
                type = "img"; u = attr(tag, "src")
                if (attr(tag, "srcset") != "" && attr(tag, "sizes") != "")
                    u = pick(attr(tag, "srcset"), attr(tag, "sizes"))
            }
            if (type != "" && u != "" && u !~ /^data:/) printf "%s\t%s\t%s\n", type, lazy, u
        }' | sort -u -t $'\t' -k3,3 -k1,1
}

absolute_url() {
    local base=$1 ref=$2
    case $ref in
        http://* | https://*) printf '%s' "$ref" ;;
        //*) printf 'http:%s' "$ref" ;;
        /*) printf '%s%s' "$base" "$ref" ;;
        *) printf '%s/%s' "$base" "$ref" ;;
    esac
}

cmd_assets() {
    local app page base type lazy ref url accept raw_s comp_s raw="$OUT/assets.jsonl"
    : >"$raw"
    for app in "${SELECTED_APPS[@]}"; do
        only_app "$app"
        base=$(url_of "$app" "")
        for page in "${ASSET_PAGES[@]}"; do
            while IFS=$'\t' read -r type lazy ref; do
                url=$(absolute_url "$base" "$ref")
                accept='*/*'
                [[ $type == img ]] && accept='image/avif,image/webp,image/apng,*/*;q=0.8'
                # The first fetch of an optimized image also warms next/image's cache.
                raw_s=$(curl -s -o /dev/null -H "Accept: $accept" -H 'Accept-Encoding: identity' \
                    -w '%{json}' "$url" || true)
                comp_s=$( (curl -s -o /dev/null -H "Accept: $accept" -H 'Accept-Encoding: br, gzip' \
                    -w '%{json}\n%{header_json}' "$url" || true) | jq -sc '.[0] + {enc: (.[1]["content-encoding"][0] // null)}')
                jq -nc --arg app "$app" --arg page "$page" --arg type "$type" --arg lazy "$lazy" \
                    --arg url "$ref" --argjson r "$raw_s" --argjson c "$comp_s" \
                    '{app: $app, page: $page, type: $type, lazy: ($lazy == "lazy"), url: $url,
                      code: $r.http_code, content_type: $r.content_type,
                      raw_bytes: $r.size_download, compressed_bytes: $c.size_download,
                      encoding: $c.enc}' >>"$raw"
            done < <(curl -s -H 'Accept-Encoding: identity' "$(url_of "$app" "$page")" | extract_refs)
            log "assets: $app $page"
        done
    done
    jq -s '
        group_by(.app + " " + .page) | map(
            . as $g
            | def total($t): ($g | map(select(.type == $t))
                | {count: length, raw: (map(.raw_bytes) | add // 0),
                   compressed: (map(.compressed_bytes) | add // 0)});
            {app: $g[0].app, page: $g[0].page,
             js: total("js"), css: total("css"), font: total("font"),
             img_all: total("img"),
             img_eager: ($g | map(select(.type == "img" and (.lazy | not)))
                | {count: length, raw: (map(.raw_bytes) | add // 0)}),
             non_200: ($g | map(select(.code != 200)) | map(.url))})' \
        "$raw" >"$OUT/assets.json"
}

oha_run() {
    local out=$1 url=$2
    shift 2
    oha --no-tui --output-format json -z "$DURATION" -c "$CONCURRENCY" "$@" "$url" >"$out"
}

cmd_load() {
    local app route slug url idle after dir="$OUT/oha" raw="$OUT/load.jsonl" mem="$OUT/memory.jsonl"
    require oha
    mkdir -p "$dir"
    : >"$raw"
    : >"$mem"
    for app in "${SELECTED_APPS[@]}"; do
        # Fresh process: idle memory after start and one request per route.
        stop_app "$app"
        only_app "$app"
        warm_up "$app"
        idle=$(mem_usage_bytes "${CONTAINER[$app]}")
        for route in "${ROUTES[@]}"; do
            slug=$(printf '%s' "$route" | tr -c 'A-Za-z0-9' '_')
            url=$(url_of "$app" "$route")
            log "load: $app $route (unlimited, ${DURATION} x ${CONCURRENCY})"
            oha_run "$dir/$app$slug-max.json" "$url"
            log "load: $app $route (${RATE} rps)"
            oha_run "$dir/$app$slug-rate.json" "$url" -q "$RATE" --latency-correction
            for mode in max rate; do
                jq -c --arg app "$app" --arg route "$route" --arg mode "$mode" '
                    {app: $app, route: $route, mode: $mode,
                     rps: .summary.requestsPerSec, success_rate: .summary.successRate,
                     p50_ms: (.latencyPercentiles.p50 * 1000),
                     p99_ms: (.latencyPercentiles.p99 * 1000),
                     status: .statusCodeDistribution, errors: .errorDistribution}' \
                    "$dir/$app$slug-$mode.json" >>"$raw"
            done
        done
        after=$(mem_usage_bytes "${CONTAINER[$app]}")
        jq -nc --arg app "$app" --argjson idle "$idle" --argjson after "$after" \
            '{app: $app, idle_bytes: $idle, after_load_bytes: $after}' >>"$mem"
    done
    jq -s . "$raw" >"$OUT/load.json"
    jq -s . "$mem" >"$OUT/memory.json"
}

# --- summary ------------------------------------------------------------------

cmd_summary() {
    local md="$OUT/summary.md"
    {
        printf '# Benchmark summary\n\n'
        if [[ -f $OUT/meta.json ]]; then
            jq -r '"- date: \(.date)\n- commit: \(.commit)\(if .dirty then " (dirty)" else "" end)\n" +
                "- host: \(.host.cpu), \(.host.nproc) threads, \(.host.mem_bytes / 1073741824 | floor) GiB, kernel \(.host.kernel)\n" +
                "- docker \(.tools.docker), \(.tools.oha), \(.tools.curl | split(" ")[0:2] | join(" "))\n" +
                "- limits: cpus 1, memory 2g (bench/compose.yml). oha: \(.params.duration) x \(.params.concurrency) / \(.params.rate) rps\n"' \
                "$OUT/meta.json"
        fi
        if [[ -f $OUT/images.json ]]; then
            printf '\n## Image size\n\n| app | image | size |\n|---|---|---|\n'
            jq -r '.[] | "| \(.app) | `\(.image)` | \(.size_bytes / 1048576 * 10 | round / 10) MiB |"' "$OUT/images.json"
        fi
        if [[ -f $OUT/coldstart.json ]]; then
            printf '\n## Cold start (docker start -> first 200 on /)\n\n'
            printf '| app | runs | median | min | max | `docker start` | first `/products/1` TTFB |\n|---|---|---|---|---|---|---|\n'
            jq -r '.[] | "| \(.app) | \(.runs) | \(.first_200_ms.median) ms | \(.first_200_ms.min) ms | \(.first_200_ms.max) ms | \(.docker_start_ms) ms | \(.first_detail_ttfb_ms) ms |"' \
                "$OUT/coldstart.json"
        fi
        if [[ -f $OUT/routes.json ]]; then
            printf '\n## Routes (median of %s, after one warm-up request)\n\n' "$ROUTE_RUNS"
            printf '| route | app | status | TTFB | total | identity | br | gzip | Cache-Control |\n|---|---|---|---|---|---|---|---|---|\n'
            jq -r 'def kb: if . == null then "-" else "\(. / 1024 * 10 | round / 10) KB" end;
                def enc(e): if e.content_encoding == null then "\(e.bytes | kb) (none)" else (e.bytes | kb) end;
                sort_by(.route, .app) | .[] |
                "| `\(.route)` | \(.app) | \(.code | map(tostring) | join(",")) | \(.ttfb_ms) ms | \(.total_ms) ms | \(.bytes | kb) | \(enc(.br)) | \(enc(.gzip)) | \(.cache_control // "-") |"' \
                "$OUT/routes.json"
        fi
        if [[ -f $OUT/stream.json ]]; then
            printf '\n## Streaming of `%s` (median; bursts = data separated by > %s ms, run 1)\n\n' "$STREAM_ROUTE" "$BURST_GAP_MS"
            printf '| app | Accept-Encoding | first data | last data | bytes | bursts (start ms: bytes) |\n|---|---|---|---|---|---|\n'
            jq -r '.[] | "| \(.app) | \(.enc) | \(.first_data_ms) ms | \(.last_data_ms) ms | \(.bytes) | \(.bursts_run1 | map("\(.start_ms): \(.bytes)") | join(", ")) |"' \
                "$OUT/stream.json"
        fi
        if [[ -f $OUT/assets.json ]]; then
            printf '\n## Subresources referenced by the HTML (raw / br-or-gzip; images: viewport %spx, DPR %s)\n\n' "$VIEWPORT" "$DPR"
            printf '| page | app | JS | CSS | images (eager) | images (all) | non-200 |\n|---|---|---|---|---|---|---|\n'
            jq -r 'def kb: "\(. / 1024 * 10 | round / 10) KB";
                sort_by(.page, .app) | .[] |
                "| `\(.page)` | \(.app) | \(.js.count) / \(.js.raw | kb) / \(.js.compressed | kb) | \(.css.count) / \(.css.raw | kb) / \(.css.compressed | kb) | \(.img_eager.count) / \(.img_eager.raw | kb) | \(.img_all.count) / \(.img_all.raw | kb) | \(.non_200 | length) |"' \
                "$OUT/assets.json"
        fi
        if [[ -f $OUT/load.json ]]; then
            printf '\n## Load (oha)\n\n| route | app | mode | RPS | p50 | p99 | success | statuses |\n|---|---|---|---|---|---|---|---|\n'
            jq -r 'def r1: (. * 10 | round) / 10;
                sort_by(.route, .mode, .app) | .[] |
                "| `\(.route)` | \(.app) | \(.mode) | \(.rps | r1) | \(.p50_ms | r1) ms | \(.p99_ms | r1) ms | \(.success_rate * 100 | r1)% | \(.status | to_entries | map("\(.key):\(.value)") | join(" ")) |"' \
                "$OUT/load.json"
        fi
        if [[ -f $OUT/memory.json ]]; then
            printf '\n## Memory (docker stats)\n\n| app | idle | after load |\n|---|---|---|\n'
            jq -r '.[] | "| \(.app) | \(.idle_bytes / 1048576 * 10 | round / 10) MiB | \(.after_load_bytes / 1048576 * 10 | round / 10) MiB |"' \
                "$OUT/memory.json"
        fi
    } >"$md"
    log "summary: $md"
}

cmd_all() {
    cmd_images
    cmd_coldstart
    cmd_routes
    cmd_stream
    cmd_assets
    cmd_load
    cmd_summary
}

# --- main -----------------------------------------------------------------------

main() {
    local cmd=${1:-}
    [[ -n $cmd ]] || { usage >&2; exit 2; }
    shift
    case $cmd in -h | --help | help) usage; exit 0 ;; esac
    while (($#)); do
        case $1 in
            -o | --out) OUT=$2; shift 2 ;;
            -a | --apps) read -r -a SELECTED_APPS <<<"$2"; shift 2 ;;
            -n | --runs) COLD_RUNS=$2; ROUTE_RUNS=$2; shift 2 ;;
            -d | --duration) DURATION=$2; shift 2 ;;
            -c | --concurrency) CONCURRENCY=$2; shift 2 ;;
            -r | --rate) RATE=$2; shift 2 ;;
            -h | --help) usage; exit 0 ;;
            *) die "unknown option: $1 (see --help)" ;;
        esac
    done
    local app
    for app in "${SELECTED_APPS[@]}"; do
        [[ -n ${PORT[$app]:-} ]] || die "unknown app: $app"
    done
    case $cmd in
        images | coldstart | routes | stream | assets | load | all)
            require
            init_out
            "cmd_$cmd"
            [[ $cmd == all ]] || cmd_summary
            ;;
        summary)
            [[ -n $OUT && -d $OUT ]] || die "summary needs -o <results dir>"
            require
            cmd_summary
            ;;
        *) die "unknown command: $cmd (see --help)" ;;
    esac
}

# Sourcing the file (for tests) defines the functions without running anything.
if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    main "$@"
fi
