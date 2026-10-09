#!/bin/bash
# GUI end-to-end test: opens a real Pharo window on your screen. Needs `stlive init` and jq.
# Usage: tests/e2e-gui.sh [path/to/stlive]
set -u
S=${1:-$(cd "$(dirname "$0")/.." && pwd)/target/release/stlive}
WORK=$(mktemp -d); cd "$WORK"
pass=0; fail=0
check() { local desc=$1 filter=$2 expected=${3:-true}; local got; got=$(echo "$out" | jq -r "$filter" 2>/dev/null)
  if [ "$got" = "$expected" ]; then pass=$((pass+1)); echo "ok   - $desc"; else fail=$((fail+1)); echo "FAIL - $desc (got: $got, expected: $expected)"; echo "       $out" | cut -c1-300; fi; }
cleanup() { $S stop --force >/dev/null 2>&1; cd /; rm -rf "$WORK"; }
trap cleanup EXIT

out=$($S start --gui); check "start --gui publishes a port" '.ok'
out=$($S eval --ui '1'); check "ui job runs" '.result.value.print' 1
out=$($S eval - <<< "Transcript show: 'gui'; cr. 1"); check "transcript capture works in the GUI image" '.result.output.transcript|rtrimstr("\n")' gui

$S class create DemoPresenter --superclass SpPresenter --ivars "label button count" --package Demo-UI >/dev/null
$S method compile 'DemoPresenter class' --protocol x - <<< $'defaultLayout\n\t^ SpBoxLayout newTopToBottom add: #label; add: #button; yourself' >/dev/null
$S method compile DemoPresenter --protocol x - <<< $'initializePresenters\n\tcount := 0.\n\tlabel := self newLabel label: \'Count: 0\'.\n\tbutton := self newButton label: \'Increment\'; action: [ count := count + 1. label label: \'Count: \', count printString ]; yourself' >/dev/null
$S method compile DemoPresenter --protocol x - <<< $'count\n\t^ count' >/dev/null
$S method compile DemoPresenter --protocol x - <<< $'labelPresenter\n\t^ label' >/dev/null
$S method compile DemoPresenter --protocol x - <<< $'buttonPresenter\n\t^ button' >/dev/null
$S method compile DemoPresenter --protocol x - <<< $'initializeWindow: aWindow\n\taWindow title: \'Demo\'; initialExtent: 320@120' >/dev/null

out=$($S eval --ui "| p | p := DemoPresenter new. Smalltalk at: #DemoP put: p. p open. 1"); check "open a Spec window in the UI process" '.ok'
out=$($S ui windows); check "window listed with its presenter" '[.result.windows[]|select(.title=="Demo")|.presenter]|first' DemoPresenter
out=$($S eval --ui "(Smalltalk at: #DemoP) labelPresenter label: 'Changed by stlive'. 1"); check "change a label from the UI process" '.ok'
out=$($S ui screenshot --window Demo); check "screenshot written" '.result.bytes>1000'
PNG=$(echo "$out" | jq -r .result.path); [ -s "$PNG" ] && file "$PNG" | grep -q "PNG image" && { pass=$((pass+1)); echo "ok   - file is a PNG"; } || { fail=$((fail+1)); echo "FAIL - not a PNG: $PNG"; }
out=$($S ui press Increment); check "press a button by label" '.result.pressed' Increment
out=$($S eval "(Smalltalk at: #DemoP) count"); check "button action ran" '.result.value.print' 1

# errors in the UI process: a stlive session, never a Pharo debugger; the GUI keeps running
out=$($S eval --ui '1/0'); check "error in the UI process becomes a session" '.error.session|startswith("dbg-")'
out=$($S eval "(World submorphs select: [:m | m isSystemWindow and: [m label asString includesSubstring: 'Debugger']]) size"); check "no Pharo debugger window opened" '.result.value.print' 0
out=$($S ui windows); check "UI still responds after the error" '.ok'
$S eval --ui "(Smalltalk at: #DemoP) buttonPresenter action: [ nil foo ]. 1" >/dev/null
out=$($S ui press Increment); check "failing button action becomes a session" '.error.type' MessageNotUnderstood
out=$($S eval "(World submorphs select: [:m | m isSystemWindow and: [m label asString includesSubstring: 'Debugger']]) size"); check "still no debugger window" '.result.value.print' 0
out=$($S method compile DemoPresenter --protocol x --ui - <<< $'double\n\t^ count * 2'); check "compile --ui" '.ok'
out=$($S ui press NoSuchButton); check "unknown button reports the available ones" '.error.code' no_such_button
out=$($S image save); check "image save needs no dialog" '.ok'
out=$($S debug terminate --all); check "end the sessions" '.ok'

echo; echo "passed: $pass, failed: $fail"; [ $fail -eq 0 ]
