#!/bin/bash
# End-to-end test against a real Pharo image. Needs `stlive init` done once, plus jq.
# Usage: tests/e2e.sh [path/to/stlive]
set -u
S=${1:-$(cd "$(dirname "$0")/.." && pwd)/target/release/stlive}
WORK=$(mktemp -d); cd "$WORK"
export STLIVE_DIR="$WORK/.stlive"
pass=0; fail=0
check() { # check <description> <jq filter on $out> [expected]
  local desc=$1 filter=$2 expected=${3:-true}
  local got; got=$(echo "$out" | jq -r "$filter" 2>/dev/null)
  if [ "$got" = "$expected" ]; then pass=$((pass+1)); echo "ok   - $desc"; else fail=$((fail+1)); echo "FAIL - $desc (got: $got, expected: $expected)"; echo "       $out" | cut -c1-300; fi
}
cleanup() { $S stop --force >/dev/null 2>&1; cd /; rm -rf "$WORK"; }
trap cleanup EXIT

out=$($S start); check "start" '.ok'
out=$($S ping); check "ping" '.result.pong'
out=$($S eval '3 + 4'); check "eval returns value" '.result.value.print' 7
out=$($S eval - <<< "Transcript show: 'hi'; cr. 1"); check "transcript captured separately" '.result.output.transcript|rtrimstr("\n")' hi

# --- error -> persistent session
$S class create Cart --ivars "items taxRate" --package Demo >/dev/null
$S method compile Cart --protocol t - <<< $'setUp\n\titems := OrderedCollection new' >/dev/null
$S method compile Cart --protocol t - <<< $'add: price\n\titems add: price' >/dev/null
$S method compile Cart --protocol t - <<< $'total\n\t| sum |\n\tsum := items inject: 0 into: [ :acc :each | acc + each ].\n\t^ sum * (1 + taxRate)' >/dev/null
out=$($S eval '| c | c := Cart new. c setUp. c add: 10. c add: 5. c total'); check "failure becomes a session" '.error.session|startswith("dbg-")'
SESSION=$(echo "$out" | jq -r .error.session)
check "first application frame found" '.error.first_application_frame' 3
out=$($S debug frames $SESSION --limit 5); check "frames paginated" '.result.frames|length' 5
out=$($S debug frame $SESSION 3); check "frame label" '.result.label' 'Cart>>#total'
check "temporary classified" '.result.variables[0].kind' temporary
RECV=$(echo "$out" | jq -r .result.receiver.ref)
out=$($S obj var $RECV items); check "follow reference" '.result.value.class' OrderedCollection
out=$($S obj show "$RECV" ); check "instance variables listed" '.result.instance_variables|length' 2
out=$($S debug eval $SESSION --frame 3 'sum + 1'); check "eval inside a frame sees locals" '.result.value.print' 16

# --- fix, stale detection, resume vs rerun vs restart
out=$($S method compile Cart - <<< $'total\n\t| sum |\n\tsum := items inject: 0 into: [ :acc :each | acc + each ].\n\t^ sum * (1 + (taxRate ifNil: [ 0 ]))')
check "compile reports stale frame" '.result.stale_frames[0].session' "$SESSION"
out=$($S debug resume $SESSION); check "resume does not pretend a repair" '.ok' false
out=$($S debug rerun $SESSION); check "rerun succeeds on fresh execution" '.result.fresh_execution' true
check "rerun value" '.result.value.print' 15
out=$($S eval '| c | c := Cart new. c setUp. c add: 10. c total' ); check "plain re-eval ok" '.result.value.print' 10

# --- restart a frame with new code
$S method compile Cart - <<< $'total\n\t| sum |\n\tsum := items inject: 0 into: [ :acc :each | acc + each ].\n\t^ sum * (1 + taxRate)' >/dev/null
out=$($S eval '| c | c := Cart new. c setUp. c add: 10. c total'); S2=$(echo "$out" | jq -r .error.session)
$S method compile Cart - <<< $'total\n\t| sum |\n\tsum := items inject: 0 into: [ :acc :each | acc + each ].\n\t^ sum * (1 + (taxRate ifNil: [ 0 ]))' >/dev/null
out=$($S debug restart $S2 --frame 3); check "restart completes with new code" '.result.value.print' 10
check "restart is not claimed as verified fix" '.result.fix_verified' false

# --- tests, grouping, session cleanup
$S class create CartTest --superclass TestCase --package Demo-Tests >/dev/null
for n in 1 2; do $S method compile CartTest --protocol t - <<< "testNil$n"$'\n\tself assert: Cart new total equals: 0' >/dev/null; done
$S method compile CartTest --protocol t - <<< $'testOk\n\tself assert: true' >/dev/null
out=$($S test run CartTest); check "tests: counts" '[.result.ran,.result.passed,.result.failed]|join(",")' "3,1,2"
check "tests: one group for one cause" '.result.groups|length' 1
check "tests: one representative session kept" '.result.groups[0].sessions|length' 1
out=$($S test list 'Demo*'); check "test list" '.result.test_classes[0].name' CartTest

# --- timeouts, recursion
out=$($S eval '[ true ] whileTrue: [ 1 + 1 ]' --timeout 500); check "endless loop becomes a timeout session" '.error.kind' timeout
out=$($S ping); check "image still responsive after timeout" '.ok'
$S class create Rec >/dev/null
$S method compile Rec - <<< $'a: n\n\t^ self b: n' >/dev/null
$S method compile Rec - <<< $'b: n\n\t^ self a: n' >/dev/null
out=$($S eval 'Rec new a: 1' --timeout 1500); check "recursion pattern detected" '.error.recursion.cycle|join(",")' 'Rec>>#b:,Rec>>#a:'
$S debug terminate --all >/dev/null

# --- large data stays paged
out=$($S eval '(1 to: 1000000) asOrderedCollection'); REF=$(echo "$out" | jq -r .result.value.ref)
out=$($S obj items "$REF" --offset 999995 --limit 10); check "collection paging" '.result.items|length' 5

# --- write back to Tonel with a minimal diff
mkdir -p src/Demo
cat > src/Demo/Cart.class.st <<'ST'
Class {
	#name : 'Cart',
	#superclass : 'Object',
	#instVars : [
		'items',
		'taxRate'
	],
	#package : 'Demo'
}

{ #category : 't' }
Cart >> add: price [
	items add: price
]
ST
out=$($S save --package Demo --dir src); check "save ok" '.ok'
check "existing method untouched, others added" '.result.files[0].added|length' 2
grep -q "ifNil: \[ 0 \]" src/Demo/Cart.class.st && { pass=$((pass+1)); echo "ok   - tonel contains the fix"; } || { fail=$((fail+1)); echo "FAIL - tonel missing fix"; }
out=$($S changes list); check "nothing unsaved afterwards for Demo" '.result.unsaved_by_package.Demo' null

# --- stale references after restart
out=$($S eval 'Object new'); OLD=$(echo "$out" | jq -r .result.value.ref)
$S stop --force >/dev/null; $S start >/dev/null
out=$($S obj show "$OLD"); check "stale reference detected after restart" '.error.code' stale_ref
out=$($S stop); check "stop needs --force" '.error.code' confirmation_required

echo; echo "passed: $pass, failed: $fail"; [ $fail -eq 0 ]
