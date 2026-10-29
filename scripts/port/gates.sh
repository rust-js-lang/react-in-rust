#!/bin/sh
# scripts/port/gates.sh <tag> [--server | --server-only] [extra jest paths...]
#
# Runs the port's gates (React's jest suites on the generated JS) from the
# repository root, each under `timeout 300`, and prints one summary line per
# gate; the logs are $PORT_LOGS/<tag>-<gate>.log. Compare the summary with
# PORTING.md's expected numbers. Extra jest paths run as one more gate, "extra".
#
#   --server       also run the server-side gates (Fizz, Flight, react-server, react-client)
#   --server-only  run only the server-side gates
#
# Environment: PORT_LOGS (default target/port/logs), PORT_WORKERS (jest's
# --maxWorkers, default 4; lower it when the machine is shared).
set -u
cd "$(dirname "$0")/../.." || exit 1
[ $# -ge 1 ] || { sed -n '2,13p' "$0"; exit 2; }
T=$1; shift
L=${PORT_LOGS:-target/port/logs}; W=${PORT_WORKERS:-4}
mkdir -p "$L"
client=1; server=0
case "${1:-}" in
  --server) server=1; shift ;;
  --server-only) server=1; client=0; shift ;;
esac

run() {
  n=$1; shift
  timeout 300 yarn test "$@" --maxWorkers="$W" > "$L/$T-$n.log" 2>&1
  s=$?
  [ $s -eq 124 ] && echo "$n: TIMEOUT after 300s (split it, never raise the limit)" && return
  echo "$n: $(grep -E '^Test Suites:' "$L/$T-$n.log" | tail -1) | $(grep -E '^Tests:' "$L/$T-$n.log" | tail -1)"
}

# The events and forms suites every react-dom-bindings change runs.
EV=$(tr -s ' \n' ' ' < scripts/port/ev.txt)

if [ $client = 1 ]; then
  run dev packages/react-reconciler packages/react/src packages/shared
  run prod --prod packages/react-reconciler packages/react/src packages/shared
  run dom react-dom/src/__tests__/ReactDOM
  run misc PerformanceTrack Profiler TestSelectors ViewTransition ErrorBoundaries Activity SuspenseList Fragment
  run sched packages/scheduler
  run schedwww -r=www-modern packages/scheduler   # profiling on: SchedulerProfiling-test
  run domrest packages/react-dom/src/__tests__ --testPathIgnorePatterns 'ReactDOM[A-Z]'
  # shellcheck disable=SC2086
  run ev $EV
fi

if [ $server = 1 ]; then
  run fizz react-dom/src/__tests__/ReactDOMFizz
  run ssr react-dom/src/__tests__/ReactDOMServerIntegration
  run float react-dom/src/__tests__/ReactDOMFloat react-dom/src/__tests__/ReactDOMLegacyFloat
  run ssrmisc react-dom/src/__tests__/ReactServerRendering react-dom/src/__tests__/ReactDOMServerLifecycles \
    react-dom/src/__tests__/ReactDOMInReactServer react-dom/src/__tests__/ReactClassComponentPropResolutionFizz \
    packages/react-markup packages/use-sync-external-store/src/__tests__/useSyncExternalStoreShimServer
  run server packages/react-server/src/__tests__
  run client packages/react-client/src/__tests__
  run webpack packages/react-server-dom-webpack/src/__tests__
  run turbopack packages/react-server-dom-turbopack/src/__tests__
  run fb packages/react-server-dom-fb/src/__tests__
fi

[ $# -gt 0 ] && run extra "$@"

# A JS label rust-js made (bb12:, break bb12) holds a module: there must be none.
grep -rnE 'break bb|^\s*bb[0-9]+:' --include='*.js' packages/react packages/react-reconciler/src \
  packages/react-dom-bindings/src packages/scheduler/src packages/shared | grep -v node_modules | head -5
echo done
