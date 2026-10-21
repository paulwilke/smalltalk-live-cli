#!/bin/bash
# End-to-end test against a real Pharo image. Needs `stlive init` done once, plus jq.
# Usage: tests/e2e.sh [path/to/stlive]
set -u
BIN=${1:-$(cd "$(dirname "$0")/.." && pwd)/target/release/stlive}
stlive_bin() { "$BIN" "$@"; }; S=stlive_bin   # the path may contain spaces; $S stays usable unquoted
WORK=$(mktemp -d); cd "$WORK"
export STLIVE_DIR="$WORK/.stlive"
export STLIVE_HOME="${STLIVE_HOME:-$HOME/.stlive}"   # the template; STLIVE_DIR alone would be used as home (0.7)
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
out=$($S debug inspect $SESSION); check "inspect has a failure summary like test run" ".result.failure.first_application_frame.label" "Cart>>#total"
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
out=$($S eval 'Rec new a: 1' --timeout 1500); check "recursion pattern detected" '.error.recursion.cycle|sort|join(",")' 'Rec>>#a:,Rec>>#b:'
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

# --- diagnostics added in 0.2
out=$($S method compile Cart - <<< $'zork\n\t^ NoSuchClassAnywhere new'); check "compile error names the undeclared variable" '.error.variable' NoSuchClassAnywhere
check "compile error has line and column" '[.error.line,.error.column]|join(":")' '2:4'
out=$($S eval 'String new: 300 withAll: $x' --full); check "eval --full returns whole text" '.result.text_total' 302
out=$($S eval '3 zork' --no-session); check "eval --no-session keeps no session" '.error.session_ended'
out=$($S eval '| a b | a := OrderedCollection new. b := OrderedCollection with: a. a add: b. a'); G=$(echo "$out" | jq -r .result.value.ref)
out=$($S obj graph "$G" --depth 3); check "object graph finds the cycle" '.result.cycles' 1
out=$($S obj referrers "$G"); check "referrers found" '.result.total>=1'
out=$($S class create Bar --ivars a --class-ivars count --class-vars Registry --comment 'A bar.' --package Demo); check "class create with class side" '.result.class_instance_variables[0]' count
out=$($S save --package Demo --dir src); check "save writes class-side definition" '.ok'
grep -q "classInstVars" src/Demo/Bar.class.st && { pass=$((pass+1)); echo "ok   - tonel has classInstVars"; } || { fail=$((fail+1)); echo "FAIL - tonel lacks classInstVars"; }
FRESH=$($S eval '3 zork' | jq -r .error.session)
out=$($S debug locals "$FRESH" 0); check "frame accepted positionally" '.ok'
STLIVE_LOG="$WORK/calls.jsonl" STLIVE_TAG=e2e $S eval '1' >/dev/null
check_log=$(tail -1 "$WORK/calls.jsonl" | jq -r '[.tag,.command,.ok]|join(",")'); out="{\"v\":\"$check_log\"}"; check "call log written with tag" '.v' 'e2e,eval,true'

# --- changes made by eval are recorded too and reach the Tonel files
$S class create Evl --ivars a --package Demo >/dev/null
$S eval - <<< "Evl addInstVarNamed: 'b'. Evl compile: 'two ^ 2' classified: 'x'. 1" >/dev/null
out=$($S changes list); check "eval-made method change recorded" '[.result.changes[]|select(.origin=="eval" and .selector=="two")]|length' 1
out=$($S save --package Demo --dir src); check "save includes eval-made changes" '.result.files|map(select(.file|endswith("Evl.class.st")))|length' 1
grep -q "two \[" src/Demo/Evl.class.st && grep -q "'b'" src/Demo/Evl.class.st && { pass=$((pass+1)); echo "ok   - tonel has eval-made method and ivar, normalised"; } || { fail=$((fail+1)); echo "FAIL - eval-made changes not in tonel"; cat src/Demo/Evl.class.st; }
# a class comment changed by eval replaces the comment block of the existing file (quotes escaped)
$S eval "Evl comment: 'Commented by eval, with \"quotes\".'" >/dev/null
out=$($S save --package Demo --dir src); check "save includes eval-made class comment" '.result.files|map(select(.file|endswith("Evl.class.st")))|length' 1
[ "$(sed -n 2p src/Demo/Evl.class.st)" = 'Commented by eval, with ""quotes"".' ] && { pass=$((pass+1)); echo "ok   - tonel has the eval-made class comment, quotes escaped"; } || { fail=$((fail+1)); echo "FAIL - eval-made class comment not in tonel"; head -4 src/Demo/Evl.class.st; }
out=$($S eval '3 zork' --no-source); check "eval --no-source drops the statement" '.error.top.statement' null
out=$($S method remove Cart total); check "method remove error names --force" '.error.message|contains("--force")'
# multi-line string literals: compiled here they hold CR like a method loaded from Tonel; recorded, shown and saved with LF
$S class create Banner --package Demo >/dev/null
out=$($S method compile Banner --protocol t - <<< $'text\n\t^ \'first line\nsecond line\''); BIDX=$(echo "$out" | jq -r .result.change_index)
out=$($S eval 'Banner new text includes: Character cr'); check "multi-line literal has CR like a Tonel-loaded method" '.result.value.print' true
out=$($S eval 'Banner new text includes: Character lf'); check "multi-line literal has no LF" '.result.value.print' false
out=$($S method show 'Banner>>text'); check "method show source has LF only" '[(.result.source|contains("\r")), (.result.source|contains("\n"))]|join(",")' 'false,true'
out=$($S changes show "$BIDX"); check "recorded change has LF only" '[(.result.new_source|contains("\r")), (.result.new_source|contains("\n"))]|join(",")' 'false,true'
out=$($S save --package Demo --dir src); check "save of the multi-line method" '.ok'
grep -q "second line'" src/Demo/Banner.class.st && ! grep -q $'\r' src/Demo/Banner.class.st && { pass=$((pass+1)); echo "ok   - tonel has the multi-line method with LF line endings only"; } || { fail=$((fail+1)); echo "FAIL - tonel line endings"; od -c src/Demo/Banner.class.st | head -20; }
out=$($S method compile Banner - <<< $'broken\n\t| s |\n\ts := NoSuchBannerThing new.\n\t^ \'first\nsecond\''); check "compile error in a multi-line method names the variable" '.error.variable' NoSuchBannerThing
check "compile error in a multi-line method has line 3" '[.error.line,.error.column]|join(":")' '3:7'

# --- attach the server to an image that is already running
PORT=$(jq -r .port .stlive/default.port.json)
out=$($S -i second attach --port "$PORT"); check "attach to a running image" '.result.attached'
out=$($S -i second eval '6 * 7'); check "attached instance answers" '.result.value.print' 42

# --- stale references after restart
out=$($S eval 'Object new'); OLD=$(echo "$out" | jq -r .result.value.ref)
$S stop --force >/dev/null; $S start >/dev/null
out=$($S obj show "$OLD"); check "stale reference detected after restart" '.error.code' stale_ref
out=$($S stop); check "stop needs --force" '.error.code' confirmation_required

# --- 0.6: recording survives a failing test run (suppression is per process)
$S class create Reg --superclass TestCase --package Demo-Reg >/dev/null
$S method compile Reg --protocol t - <<< $'testRed\n\tself assert: false' >/dev/null
$S test run Reg >/dev/null
$S eval - <<< "Reg compile: 'later ^ 1' classified: 'x'. 1" >/dev/null
out=$($S changes list); check "eval-made change recorded after a red test left a session behind" '[.result.changes[]|select(.selector=="later")]|length' 1
$S debug terminate --all >/dev/null

# --- drift / save --verify
mkdir -p dsrc
out=$($S save --package Demo-Reg --dir dsrc --verify); check "save --verify reports no drift" '.ok'
out=$($S drift --package Demo-Reg --dir dsrc); check "drift: image and files agree" '.result.drift' false
perl -0pi -e 's/\{ #category : .x. \}\nReg >> later \[.*?\n\]\n?//s' dsrc/Demo-Reg/Reg.class.st
out=$($S drift --package Demo-Reg --dir dsrc); check "drift detects a method missing in the files" '.result.packages[0].missing_in_files|join(",")' 'Reg>>later'
$S drift --package Demo-Reg --dir dsrc >/dev/null; [ $? -eq 1 ] && { pass=$((pass+1)); echo "ok   - drift exits 1"; } || { fail=$((fail+1)); echo "FAIL - drift exit code"; }

# --- did you mean / truncation hint
out=$($S eval '12 printString: 5 paddedWith: $0 to: 5' --no-session); check "unknown selector gets suggestions" '[.error.did_you_mean[].selector]|index("printPaddedWith:to:")!=null'
out=$($S eval '(1 to: 100) asOrderedCollection'); check "truncated print says how to get the rest" '.result.value.hint|contains("obj text")'

# --- stlive run: fresh image, sources from Tonel, clean stdout, UTF-8, args, exit code
mkdir -p runsrc/RunApp
printf "Package { #name : 'RunApp' }\n" > runsrc/RunApp/package.st
cat > runsrc/RunApp/RunGreeter.class.st <<'ST'
Class {
	#name : 'RunGreeter',
	#superclass : 'Object',
	#package : 'RunApp'
}

{ #category : 'x' }
RunGreeter class >> greet: aName [
	^ 'Grüße, ' , aName , ' (21 °C)'
]
ST
RUNOUT=$($S run --load runsrc --eval "RunGreeter greet: StLiveRun arguments first" -- Welt 2>/dev/null)
out="{\"v\":\"$RUNOUT\"}"; check "run: result on stdout, UTF-8 intact, args passed" '.v' 'Grüße, Welt (21 °C)'
RUNOUT=$($S run --load runsrc --eval "Transcript show: 'noise'. Warning signal: 'w'. 42" 2>/dev/null)
out="{\"v\":\"$RUNOUT\"}"; check "run: stdout carries only the result" '.v' 42
$S run --eval "StLiveRun exit: 3" >/dev/null 2>&1; [ $? -eq 3 ] && { pass=$((pass+1)); echo "ok   - run passes the exit code through"; } || { fail=$((fail+1)); echo "FAIL - run exit code"; }
$S run --eval "1 zork" >/dev/null 2>"$WORK/run.err"; RC=$?
{ [ $RC -eq 1 ] && grep -q "zork" "$WORK/run.err"; } && { pass=$((pass+1)); echo "ok   - run: errors go to stderr with exit code 1"; } || { fail=$((fail+1)); echo "FAIL - run error handling (rc=$RC)"; }

# --- 0.6.1: NeoJSON in the template, every command is logged
RUNOUT=$($S run --eval "NeoJSONWriter toString: (OrderedCollection with: #() with: #() with: 'ä')" 2>/dev/null)
out="{\"v\":\"$(echo "$RUNOUT" | sed 's/"/\\"/g')\"}"; check "NeoJSON is in the image (shared empty literals are fine)" '.v|contains("ä")'
STLIVE_LOG="$WORK/all.jsonl" STLIVE_TAG=e2e $S run --eval 1 >/dev/null 2>&1
STLIVE_LOG="$WORK/all.jsonl" $S drift --all --dir dsrc >/dev/null 2>&1
out="{\"v\":\"$(jq -r .command "$WORK/all.jsonl" | tr "\n" ",")\"}"; check "run and drift appear in the call log" '.v' "run,drift,"

# --- 0.7: STLIVE_DIR is the home when STLIVE_HOME is unset (a forgotten variable never touches ~/.stlive)
out=$(env -u STLIVE_HOME STLIVE_DIR="$WORK/empty-state" "$BIN" -i nohome start 2>/dev/null); check "without STLIVE_HOME the template comes from STLIVE_DIR" '.error.code' not_initialized
[ ! -e "$HOME/.stlive/nohome.config.json" ] && { pass=$((pass+1)); echo "ok   - ~/.stlive untouched"; } || { fail=$((fail+1)); echo "FAIL - ~/.stlive was written"; }

# --- 0.7: inherited test methods (abstract superclass) and skipped tests
$S class create AbstractThingTest --superclass TestCase --package Demo-Tests >/dev/null
$S method compile 'AbstractThingTest class' --protocol t - <<< $'isAbstract\n\t^ self == AbstractThingTest' >/dev/null
$S method compile AbstractThingTest --protocol t - <<< $'testInherited\n\tself assert: self thing > 0' >/dev/null
$S method compile AbstractThingTest --protocol t - <<< $'thing\n\t^ self subclassResponsibility' >/dev/null
$S class create ConcreteThingTest --superclass AbstractThingTest --package Demo-Tests >/dev/null
$S method compile ConcreteThingTest --protocol t - <<< $'thing\n\t^ 1' >/dev/null
$S method compile ConcreteThingTest --protocol t - <<< $'testOwn\n\tself assert: true' >/dev/null
$S method compile ConcreteThingTest --protocol t - <<< $'testSkipped\n\tself skip: \'not today\'' >/dev/null
out=$($S test run ConcreteThingTest); check "inherited test runs, skip counted separately" '[.result.ran,.result.passed,.result.failed,.result.skipped]|join(",")' "3,2,0,1"
check "skipped tests are named" '.result.skipped_tests[0]' 'ConcreteThingTest>>testSkipped'
out=$($S test run AbstractThingTest); check "abstract class runs its concrete subclasses" '.result.ran' 3
out=$($S test list 'Demo-Tests'); check "test list counts what SUnit runs" '.result.test_classes[0].tests' 3
$S test run ConcreteThingTest >/dev/null; [ $? -eq 0 ] && { pass=$((pass+1)); echo "ok   - skips do not fail the run by default"; } || { fail=$((fail+1)); echo "FAIL - skip exit code without flag"; }
$S test run ConcreteThingTest --skips-fail >/dev/null; [ $? -eq 1 ] && { pass=$((pass+1)); echo "ok   - --skips-fail exits 1"; } || { fail=$((fail+1)); echo "FAIL - --skips-fail exit code"; }

# --- 0.7: a ParametrizedTestCase runs once per parameter combination, reported per case
$S class create ParamDemoTest --superclass ParametrizedTestCase --ivars n --package Demo-Tests >/dev/null
$S method compile 'ParamDemoTest class' --protocol t - <<< $'testParameters\n\t^ ParametrizedTestMatrix new forSelector: #n addOptions: #(1 2 3); yourself' >/dev/null
$S method compile ParamDemoTest --protocol t - <<< $'n: aNumber\n\tn := aNumber' >/dev/null
$S method compile ParamDemoTest --protocol t - <<< $'testEven\n\tself assert: n even' >/dev/null
$S method compile ParamDemoTest --protocol t - <<< $'testPositive\n\tself assert: n > 0' >/dev/null
out=$($S test run ParamDemoTest); check "parametrized: every combination runs" '[.result.ran,.result.passed,.result.failed]|join(",")' "6,4,2"
check "parametrized: cases are reported with their parameters" '[.result.groups[].tests[]]|sort|join(";")' "ParamDemoTest>>testEven (#n->1);ParamDemoTest>>testEven (#n->3)"
out=$($S test run 'ParamDemoTest>>testEven'); check "Class>>selector runs all combinations of that selector" '.result.ran' 3
out=$($S test run 'ParamDemoTest>>testEven (#n->3)'); check "one combination can be addressed (and re-run)" '[.result.ran,.result.failed]|join(",")' "1,1"
PSESS=$(echo "$out" | jq -r '.result.groups[0].sessions[0]')
out=$($S debug rerun "$PSESS"); check "rerun of a parametrized case keeps its parameters" '[.result.groups[].tests[]]|join(";")' "ParamDemoTest>>testEven (#n->3)"
out=$($S test list ParamDemo); check "test list counts parametrized cases" '.result.test_classes[0].tests' 6

# --- 0.7: more failing tests than the image keeps sessions for
$S class create ManyRed --superclass TestCase --package Demo-Tests >/dev/null
$S eval --no-session - <<< "1 to: 70 do: [ :i | ManyRed compile: 'testRed' , i printString , ' self assert: ' , i printString , ' > 1000' classified: 't' ]. 1" >/dev/null
out=$($S test run ManyRed); check "70 failing tests: run completes" '[.ok,.result.failed]|join(",")' "true,70"
out=$($S debug list); check "at most 60 sessions kept" '.result.sessions|length<=60'
out=$($S test run ManyRed --discard-sessions); check "discard-sessions ends every session" '[.result.groups[].sessions|length]|add' 0
$S debug terminate --all >/dev/null

# --- 0.7: traits in save and drift
mkdir -p tsrc
out=$($S class create TGreeter --trait --ivars greeting --package TraitDemo); check "class create --trait" '.result.kind' trait
out=$($S class create Greeter --traits 'TGreeter' --ivars name --package TraitDemo); check "class create --traits" '.result.traits' TGreeter
$S method compile TGreeter --protocol g - <<< $'greet\n\t^ \'hi \' , self name' >/dev/null
$S method compile 'TGreeter classSide' --protocol g - <<< $'default\n\t^ self new' >/dev/null
$S method compile Greeter --protocol a - <<< $'name\n\t^ name ifNil: [ \'world\' ]' >/dev/null
out=$($S eval 'Greeter default greet'); check "trait methods work" '.result.value.print' "'hi world'"
out=$($S save --package TraitDemo --dir tsrc --verify); check "save --verify with a trait: no drift" '.ok'
grep -q "^Trait {" tsrc/TraitDemo/TGreeter.trait.st && grep -q "TGreeter classSide >> default" tsrc/TraitDemo/TGreeter.trait.st && grep -q "#traits : 'TGreeter'" tsrc/TraitDemo/Greeter.class.st && { pass=$((pass+1)); echo "ok   - tonel trait file and #traits line written"; } || { fail=$((fail+1)); echo "FAIL - trait tonel"; ls tsrc/TraitDemo; }
$S eval --no-session - <<< "TGreeter compile: 'shout ^ self greet asUppercase' classified: 'g'. Greeter addInstVarNamed: 'extra'. 1" >/dev/null
out=$($S save --package TraitDemo --dir tsrc --verify); check "eval-made trait method and ivar saved without drift" '.ok'
grep -q "#classTraits : 'TGreeter classTrait'" tsrc/TraitDemo/Greeter.class.st && grep -q "'extra'" tsrc/TraitDemo/Greeter.class.st && { pass=$((pass+1)); echo "ok   - class definition patch keeps the traits lines"; } || { fail=$((fail+1)); echo "FAIL - traits lines lost"; head -12 tsrc/TraitDemo/Greeter.class.st; }
RUNOUT=$($S run --load tsrc --eval "Greeter default shout" 2>/dev/null); out="{\"v\":\"$RUNOUT\"}"; check "saved trait package loads in a fresh image" '.v' 'HI WORLD'

# --- 0.7: re-loading a baseline updates loaded packages, modified ones and the baseline itself
mkdir -p lsrc/BaselineOfLoadDemo lsrc/LoadDemo ldep/BaselineOfLoadDep ldep/LoadDep
printf "Package { #name : 'BaselineOfLoadDemo' }\n" > lsrc/BaselineOfLoadDemo/package.st
printf "Package { #name : 'LoadDemo' }\n" > lsrc/LoadDemo/package.st
printf "Package { #name : 'BaselineOfLoadDep' }\n" > ldep/BaselineOfLoadDep/package.st
printf "Package { #name : 'LoadDep' }\n" > ldep/LoadDep/package.st
baseline() { # baseline <file> <name> <body>
  printf "Class {\n\t#name : '%s',\n\t#superclass : 'BaselineOf',\n\t#package : '%s'\n}\n\n{ #category : 'baselines' }\n%s >> baseline: spec [\n\t<baseline>\n\tspec for: #common do: [\n\t\t%s ]\n]\n" "$2" "$2" "$2" "$3" > "$1"; }
baseline lsrc/BaselineOfLoadDemo/BaselineOfLoadDemo.class.st BaselineOfLoadDemo "spec package: 'LoadDemo'"
baseline ldep/BaselineOfLoadDep/BaselineOfLoadDep.class.st BaselineOfLoadDep "spec package: 'LoadDep'"
printf "Class {\n\t#name : 'LoadThing',\n\t#superclass : 'Object',\n\t#package : 'LoadDemo'\n}\n\n{ #category : 'x' }\nLoadThing >> answer [\n\t^ 1\n]\n" > lsrc/LoadDemo/LoadThing.class.st
printf "Class {\n\t#name : 'LoadDepThing',\n\t#superclass : 'Object',\n\t#package : 'LoadDep'\n}\n\n{ #category : 'x' }\nLoadDepThing >> answer [\n\t^ 10\n]\n" > ldep/LoadDep/LoadDepThing.class.st
out=$($S load LoadDemo --repository "tonel://$WORK/lsrc"); check "load a tonel baseline" '.result.loaded.new_package_count' 2
$S method compile LoadThing - <<< $'answer\n\t^ 99' >/dev/null
sed -i.bak 's/\^ 1$/^ 2/' lsrc/LoadDemo/LoadThing.class.st
out=$($S load LoadDemo --repository "tonel://$WORK/lsrc"); check "re-load overwrites a package modified in the image" '.result.loaded.modified_packages_overwritten[0]' LoadDemo
out=$($S eval 'LoadThing new answer'); check "the file version is in the image" '.result.value.print' 2
out=$($S changes list); check "superseded unsaved changes are not written back later" '.result.unsaved_by_package.LoadDemo' null
baseline lsrc/BaselineOfLoadDemo/BaselineOfLoadDemo.class.st BaselineOfLoadDemo "spec baseline: 'LoadDep' with: [ spec repository: 'tonel://$WORK/ldep' ]. spec package: 'LoadDemo' with: [ spec requires: #('LoadDep') ]"
out=$($S load LoadDemo --repository "tonel://$WORK/lsrc"); check "a changed baseline is reloaded (new dependency appears)" '.result.loaded.new_packages|index("LoadDep")!=null'
sed -i.bak 's/\^ 10$/^ 20/' ldep/LoadDep/LoadDepThing.class.st
$S load LoadDemo --repository "tonel://$WORK/lsrc" >/dev/null
out=$($S eval 'LoadDepThing new answer'); check "dependency package changes arrive through the top-level baseline" '.result.value.print' 20

# --- 0.7.1: packages the baseline dropped (stale), groups that do not re-apply a package, load --prune
mkdir -p psrc/BaselineOfPrune psrc/PruneCore psrc/PruneGone psrc/PruneDirty psrc/PruneTests
for p in BaselineOfPrune PruneCore PruneGone PruneDirty PruneTests; do printf "Package { #name : '%s' }\n" $p > psrc/$p/package.st; done
for c in PruneCore:PruneCoreThing PruneGone:PruneGoneThing PruneDirty:PruneDirtyThing PruneTests:PruneTestsThing; do
  printf "Class {\n\t#name : '%s',\n\t#superclass : 'Object',\n\t#package : '%s'\n}\n" "${c#*:}" "${c%%:*}" > "psrc/${c%%:*}/${c#*:}.class.st"; done
cat > psrc/BaselineOfPrune/BaselineOfPrune.class.st <<'ST'
Class {
	#name : 'BaselineOfPrune',
	#superclass : 'BaselineOf',
	#package : 'BaselineOfPrune'
}

{ #category : 'baselines' }
BaselineOfPrune >> baseline: spec [
	<baseline>
	spec for: #common do: [
		spec package: 'PruneCore'.
		spec package: 'PruneGone'.
		spec package: 'PruneDirty'.
		spec package: 'PruneTests' ].
	spec group: 'default' with: #('PruneCore' 'PruneGone' 'PruneDirty').
	spec group: 'Tests' with: #('default' 'PruneTests')
]
ST
out=$($S load Prune --repository "tonel://$WORK/psrc" --group Tests); check "load with the Tests group loads every package" '.result.loaded.new_package_count' 5
$S method compile PruneDirtyThing --protocol t - <<< $'dirty\n\t^ 99' >/dev/null
# the baseline drops two packages and their folders are deleted; PruneDirty has unsaved changes in the image
cat > psrc/BaselineOfPrune/BaselineOfPrune.class.st <<'ST'
Class {
	#name : 'BaselineOfPrune',
	#superclass : 'BaselineOf',
	#package : 'BaselineOfPrune'
}

{ #category : 'baselines' }
BaselineOfPrune >> baseline: spec [
	<baseline>
	spec for: #common do: [
		spec package: 'PruneCore'.
		spec package: 'PruneTests' ].
	spec group: 'default' with: #('PruneCore').
	spec group: 'Tests' with: #('default' 'PruneTests')
]
ST
rm -rf psrc/PruneGone psrc/PruneDirty
out=$($S load Prune --repository "tonel://$WORK/psrc"); check "a package the baseline dropped is reported as stale" '.result.loaded.stale_packages|join(",")' 'PruneDirty,PruneGone'
check "a package of another group is reported as not reloaded" '.result.loaded.not_reloaded_packages[0].package' PruneTests
check "the not-reloaded hint names the group that loads it" '.result.loaded.not_reloaded_packages[0].hint' 'load again with --group Tests'
check "nothing is pruned without --prune" '.result.loaded.pruned_packages|length' 0
out=$($S eval 'Smalltalk globals includesKey: #PruneGoneThing'); check "stale class still exists without --prune" '.result.value.print' true
TOTAL=$($S changes list | jq -r .result.total)
out=$($S load Prune --repository "tonel://$WORK/psrc" --prune); check "--prune unloads the stale package without unsaved changes" '.result.loaded.pruned_packages|join(",")' PruneGone
check "a stale package with unsaved changes is skipped and says why" '[.result.loaded.prune_skipped[]|select(.package=="PruneDirty")|.reason|contains("unsaved")]|join(",")' true
out=$($S eval 'Smalltalk globals includesKey: #PruneGoneThing'); check "pruned class is gone" '.result.value.print' false
out=$($S eval 'Smalltalk globals includesKey: #PruneDirtyThing'); check "skipped stale class still exists" '.result.value.print' true
out=$($S changes list); check "pruning is not recorded as changes" '.result.total' "$TOTAL"
rm -rf pdrift; cp -r psrc pdrift; rm -rf pdrift/PruneTests
out=$($S drift --all --dir psrc); check "drift --all: every listed package has its folder" '.result.packages_missing_in_files|length' 0
out=$($S drift --all --dir pdrift); check "drift --all reports a loaded package whose folder is missing" '.result.packages_missing_in_files[0].package' PruneTests
check "the missing folder is attributed to its baseline" '.result.packages_missing_in_files[0].listed_by|join(",")' BaselineOfPrune
$S drift --all --dir pdrift >/dev/null; [ $? -eq 1 ] && { pass=$((pass+1)); echo "ok   - drift --all exits 1 for a missing folder"; } || { fail=$((fail+1)); echo "FAIL - drift --all missing folder exit code"; }

# --- 0.7.1: stale records survive a restart, one-line methods
# `stop` discards unsaved state, loaded packages included: the restart is checked after `image save`
mkdir -p rsrc/BaselineOfRestart rsrc/RestartA rsrc/RestartB lfsrc
for p in BaselineOfRestart RestartA RestartB; do printf "Package { #name : '%s' }\n" $p > rsrc/$p/package.st; done
cat > rsrc/BaselineOfRestart/BaselineOfRestart.class.st <<'ST'
Class {
	#name : 'BaselineOfRestart',
	#superclass : 'BaselineOf',
	#package : 'BaselineOfRestart'
}

{ #category : 'baselines' }
BaselineOfRestart >> baseline: spec [
	<baseline>
	spec for: #common do: [
		spec package: 'RestartA'.
		spec package: 'RestartB' ]
]
ST
cat > rsrc/RestartA/RestartAThing.class.st <<'ST'
Class {
	#name : 'RestartAThing',
	#superclass : 'Object',
	#package : 'RestartA'
}

{ #category : 'x' }
RestartAThing >> one [ ^ 1 ]

{ #category : 'x' }
RestartAThing >> add: x [ ^ x + 1 ]
ST
cat > rsrc/RestartB/RestartBThing.class.st <<'ST'
Class {
	#name : 'RestartBThing',
	#superclass : 'Object',
	#package : 'RestartB'
}

{ #category : 'x' }
RestartBThing >> answer [
	^ 2
]
ST
out=$($S load Restart --repository "tonel://$WORK/rsrc"); check "one-line methods: baseline with packages A and B loads" '.result.loaded.new_package_count' 3
out=$($S drift --package RestartA --dir rsrc); check "drift is clean for loaded one-line methods" '.result.drift' false
# B leaves the baseline: stale in the load that drops it, and still stale after an image save and a restart
cat > rsrc/BaselineOfRestart/BaselineOfRestart.class.st <<'ST'
Class {
	#name : 'BaselineOfRestart',
	#superclass : 'BaselineOf',
	#package : 'BaselineOfRestart'
}

{ #category : 'baselines' }
BaselineOfRestart >> baseline: spec [
	<baseline>
	spec for: #common do: [
		spec package: 'RestartA' ]
]
ST
out=$($S load Restart --repository "tonel://$WORK/rsrc"); check "a package the baseline dropped is stale" '.result.loaded.stale_packages|join(",")' RestartB
$S image save >/dev/null; $S stop --force >/dev/null; $S start >/dev/null
out=$($S load Restart --repository "tonel://$WORK/rsrc"); check "stale record survives an image save and a restart" '.result.loaded.stale_packages|join(",")' RestartB
out=$($S load Restart --repository "tonel://$WORK/rsrc" --prune); check "--prune after the restart unloads the stale package" '.result.loaded.pruned_packages|join(",")' RestartB
# a one-line method changed in the image is saved in place; the other one-line method stays as it was
$S method compile RestartAThing --protocol x - <<< $'one\n\t^ 5' >/dev/null
out=$($S save --package RestartA --dir rsrc); check "save of a changed one-line method" '.ok'
grep -qxF $'\t^ 5' rsrc/RestartA/RestartAThing.class.st && grep -qF 'RestartAThing >> add: x [ ^ x + 1 ]' rsrc/RestartA/RestartAThing.class.st && ! grep -qF 'one [ ^ 1 ]' rsrc/RestartA/RestartAThing.class.st && { pass=$((pass+1)); echo "ok   - tonel has the one-line method saved in place, the other one-liner untouched"; } || { fail=$((fail+1)); echo "FAIL - one-line method saved in place"; cat rsrc/RestartA/RestartAThing.class.st; }
out=$($S drift --package RestartA --dir rsrc); check "drift is clean after saving the one-line method" '.result.drift' false
# eval-made method with a multi-line literal: recorded, saved and drift-checked with LF only
$S class create LfThing --package LfDemo >/dev/null
$S eval - <<< $'LfThing compile: \'lfText\n\t^ \'\'x\ny\'\'\' classified: \'x\'. 1' >/dev/null
out=$($S changes list); TOTAL=$(echo "$out" | jq -r .result.total)
out=$($S changes show "$TOTAL"); check "eval-made multi-line method is the last change, origin eval" '[.result.origin,.result.selector]|join(",")' 'eval,lfText'
check "eval-made change records LF only" '[(.result.new_source|contains("\r")), (.result.new_source|contains("\n"))]|join(",")' 'false,true'
out=$($S save --package LfDemo --dir lfsrc); check "save of the eval-made multi-line method" '.ok'
grep -qxF "y'" lfsrc/LfDemo/LfThing.class.st && ! grep -q $'\r' lfsrc/LfDemo/LfThing.class.st && { pass=$((pass+1)); echo "ok   - tonel has the eval-made method with LF line endings only"; } || { fail=$((fail+1)); echo "FAIL - eval-made method line endings"; od -c lfsrc/LfDemo/LfThing.class.st | head -20; }
out=$($S drift --package LfDemo --dir lfsrc); check "drift is clean for the eval-made multi-line method" '.result.drift' false
out=$($S eval 'LfThing new lfText includes: Character lf'); check "the live literal keeps the LF typed in eval" '.result.value.print' true

# --- 0.7: an unhandled error in a forked process no longer ends the headless image
out=$($S eval '[ 1 / 0 ] fork. (Delay forMilliseconds: 300) wait. 7'); check "forked error: eval still answers" '.result.value.print' 7
out=$($S ping); check "image survives the forked error" '.result.pong'
out=$($S image info); check "background error recorded" '.result.background_errors[0].type' ZeroDivide

# --- delivery: export an image without the stlive server
$S start >/dev/null
$S class create Shipped --package App >/dev/null
$S method compile Shipped --protocol t - <<< $'answer\n\t^ 42' >/dev/null
VM=$(jq -r .vm .stlive/default.config.json)
out=$($S image export "$WORK/dist/App.image"); check "export needs --force" '.error.code' confirmation_required
out=$($S image export "$WORK/dist/App.image" --force); check "export writes the image" '.ok'
cat > "$WORK/dist/check.st" <<ST
| out |
out := WriteStream on: String new.
out nextPutAll: (Smalltalk globals includesKey: #StLiveServer) printString; nextPutAll: ','; nextPutAll: (Smalltalk globals at: #Shipped) new answer printString.
'$WORK/dist/check.txt' asFileReference writeStreamDo: [ :s | s nextPutAll: out contents ].
Smalltalk snapshot: false andQuit: true.
ST
(cd "$WORK/dist" && "$VM" --headless App.image st check.st >/dev/null 2>&1 &)
for i in $(seq 1 40); do [ -s "$WORK/dist/check.txt" ] && break; sleep 0.5; done
out="{\"v\":\"$(cat "$WORK/dist/check.txt" 2>/dev/null)\"}"; check "exported image: server gone, application kept" '.v' "false,42"

echo; echo "passed: $pass, failed: $fail"; [ $fail -eq 0 ]
