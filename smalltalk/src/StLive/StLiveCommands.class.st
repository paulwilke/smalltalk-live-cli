"
All commands. A request { cmd: 'debug.frames', args: {...} } is dispatched to #cmdDebugFrames:.
Each command answers a response dictionary { ok, result | error }.
"
Class {
	#name : 'StLiveCommands',
	#superclass : 'Object',
	#instVars : [
		'server'
	],
	#package : 'StLive'
}

{ #category : 'instance creation' }
StLiveCommands class >> on: aServer [
	^ self new setServer: aServer
]

{ #category : 'initialization' }
StLiveCommands >> setServer: aServer [
	server := aServer
]

{ #category : 'dispatch' }
StLiveCommands >> dispatch: request [
	| cmd args selector |
	cmd := request at: 'cmd' ifAbsent: [ ^ (StLiveFailure code: 'bad_request' message: 'Missing cmd') signal ].
	args := request at: 'args' ifAbsent: [ Dictionary new ].
	selector := ('cmd' , (((cmd substrings: '._-') collect: [ :p | p capitalized ]) inject: '' into: [ :a :b | a , b ]) , ':') asSymbol.
	(self respondsTo: selector)
		ifFalse: [ ^ (StLiveFailure code: 'unknown_command' message: 'Unknown command: ' , cmd) signal ].
	^ self perform: selector with: args
]

{ #category : 'helpers' }
StLiveCommands >> m: associations [
	^ OrderedDictionary newFrom: associations
]

{ #category : 'helpers' }
StLiveCommands >> ok: aDictionary [
	^ OrderedDictionary new at: 'ok' put: true; at: 'result' put: aDictionary; yourself
]

{ #category : 'helpers' }
StLiveCommands >> fail: code message: text [
	^ (StLiveFailure code: code message: text) signal
]

{ #category : 'helpers' }
StLiveCommands >> need: key in: args [
	^ (args at: key ifAbsent: [ nil ]) ifNil: [ self fail: 'missing_argument' message: 'Missing argument: ' , key ]
]

{ #category : 'helpers' }
StLiveCommands >> int: key in: args default: default [
	^ (args at: key ifAbsent: [ nil ]) ifNil: [ default ] ifNotNil: [ :v | v isInteger ifTrue: [ v ] ifFalse: [ v asString asInteger ifNil: [ self fail: 'bad_argument' message: key , ' must be an integer' ] ] ]
]

{ #category : 'helpers' }
StLiveCommands >> summary: anObject [
	^ server describer summary: anObject
]

{ #category : 'helpers' }
StLiveCommands >> classNamed: aString [
	"'Foo' or 'Foo class'."
	| name meta cls |
	name := aString trimBoth.
	meta := name endsWith: ' class'.
	meta ifTrue: [ name := (name copyFrom: 1 to: name size - 6) trimBoth ].
	cls := Smalltalk globals at: name asSymbol ifAbsent: [ self fail: 'unknown_class' message: 'No class named ' , name ].
	cls isBehavior ifFalse: [ self fail: 'unknown_class' message: name , ' is not a class' ].
	^ meta ifTrue: [ cls class ] ifFalse: [ cls ]
]

{ #category : 'helpers' }
StLiveCommands >> lf: aString [
	"Source text with LF line endings (Pharo stores CR; files and diffs want LF)."
	aString ifNil: [ ^ nil ].
	^ (aString copyReplaceAll: String crlf with: String lf) copyReplaceAll: String cr with: String lf
]

{ #category : 'helpers' }
StLiveCommands >> allBehaviorsDo: aBlock [
	Smalltalk globals allClassesAndTraits do: [ :c | aBlock value: c. aBlock value: c class ]
]

{ #category : 'helpers' }
StLiveCommands >> originCmd: cmd args: args [
	^ self m: { 'cmd' -> cmd. 'args' -> args }
]

{ #category : 'helpers' }
StLiveCommands >> compileDoit: code receiver: receiver context: context [
	^ [ | compiler |
		compiler := OpalCompiler new source: code; receiver: receiver; noPattern: true; yourself.
		context ifNotNil: [ compiler context: context ].
		compiler compile ]
		on: OCCodeError
		do: [ :e |
			(StLiveFailure
				code: 'syntax_error'
				message: e messageText asString
				details: (self m: {
					'position' -> e location.
					'line' -> ((code copyFrom: 1 to: (e location min: code size)) lines size max: 1) })) signal ]
]

{ #category : 'jobs' }
StLiveCommands >> sessionError: aSession [
	| d top first |
	d := OrderedDictionary new.
	d at: 'type' put: aSession typeName.
	d at: 'message' put: aSession messageText.
	d at: 'kind' put: (aSession kind == #interrupted ifTrue: [ 'timeout' ] ifFalse: [ 'exception' ]).
	d at: 'session' put: aSession id.
	d at: 'top_frame' put: 0.
	top := aSession contextAt: 0.
	top ifNotNil: [ d at: 'top' put: (server describer frame: top index: 0) ].
	first := self firstApplicationFrame: aSession.
	d at: 'first_application_frame' put: first.
	aSession exception ifNotNil: [ :ex | d at: 'exception' put: (self summary: ex) ].
	aSession continuedFrom ifNotNil: [ :c | d at: 'continued_from' put: c ].
	(self recursionOf: aSession) ifNotNil: [ :r | d at: 'recursion' put: r ].
	^ d
]

{ #category : 'jobs' }
StLiveCommands >> recursionOf: aSession [
	"Detect a repeating call pattern in the top frames: answers { period, first_frame, cycle, depth } or nil."
	| labels n |
	labels := ((aSession contextsFrom: 0 limit: 120) key collect: [ :c | server describer labelOf: c ]).
	n := labels size.
	n < 12 ifTrue: [ ^ nil ].
	1 to: 8 do: [ :period |
		0 to: (n - (period * 4)) do: [ :start |
			| repeats |
			repeats := true.
			start + 1 to: start + (period * 3) do: [ :i |
				(labels at: i) = (labels at: i + period) ifFalse: [ repeats := false ] ].
			repeats ifTrue: [
				^ self m: {
					'period' -> period.
					'first_frame' -> start.
					'cycle' -> (labels copyFrom: start + 1 to: start + period).
					'frames_seen_repeating' -> (n - start).
					'stack_depth_at_least' -> aSession frameCount.
					'note' -> 'The top of the stack repeats this cycle: likely endless recursion. The call that started it is far below; inspect the cycle frames for the argument that never changes.' } ] ] ].
	^ nil
]

{ #category : 'jobs' }
StLiveCommands >> firstApplicationFrame: aSession [
	| page |
	page := (aSession contextsFrom: 0 limit: 300) key.
	page withIndexDo: [ :ctx :i |
		(server describer originOf: ctx) = 'application' ifTrue: [ ^ i - 1 ] ].
	^ nil
]

{ #category : 'jobs' }
StLiveCommands >> awaitJob: job timeout: ms mode: mode from: previousSession [
	"Wait for the job's next event and render it. mode: first | resume | restart | return | rerun."
	| event session result response report |
	event := job waitEvent: ms.
	event == #timeout ifTrue: [ event := job interrupt ].
	report := nil.
	event == #error ifTrue: [
		session := job session.
		session awaitParked.
		previousSession ifNotNil: [ session continuedFrom: previousSession id ].
		response := OrderedDictionary new
			at: 'ok' put: false;
			at: 'mode' put: mode;
			at: 'error' put: (self sessionError: session);
			yourself.
		report := job takeReport.
		report ifNotNil: [ (response at: 'error') at: 'output' put: report ].
		^ response ].
	event == #terminated ifTrue: [
		^ OrderedDictionary new
			at: 'ok' put: false;
			at: 'mode' put: mode;
			at: 'error' put: (self m: { 'type' -> 'Terminated'. 'message' -> 'The execution was terminated.'. 'kind' -> 'terminated' });
			yourself ].
	result := self m: { 'completed' -> true. 'mode' -> mode. 'value' -> (self summary: job value) }.
	report := job takeReport.
	report ifNotNil: [ result at: 'output' put: report ].
	self annotate: result mode: mode.
	^ self ok: result
]

{ #category : 'jobs' }
StLiveCommands >> annotate: result mode: mode [
	mode = 'restart' ifTrue: [
		result at: 'fresh_execution' put: false.
		result at: 'fix_verified' put: false.
		result at: 'note' put: 'The suspended execution was unwound to the chosen frame and that method was restarted with the code now in the image; frames below it (and all object state) are unchanged. This verifies the method fix only for this execution; re-run the original operation to verify end to end.' ].
	(#(resume return) includes: mode asSymbol) ifTrue: [
		result at: 'fresh_execution' put: false.
		result at: 'fix_verified' put: false.
		result at: 'note' put: 'A previously suspended execution was continued. Its existing frames keep running (methods recompiled meanwhile only affect frames entered afterwards). Re-run the original operation to verify a fix.' ].
	mode = 'rerun' ifTrue: [
		result at: 'fresh_execution' put: true.
		result at: 'note' put: 'The original operation was executed again from scratch with the code currently in the image.' ]
]

{ #category : 'jobs' }
StLiveCommands >> runBlock: aBlock origin: origin timeout: ms mode: mode [
	| job |
	job := server newJob: aBlock origin: origin.
	job start.
	^ self awaitJob: job timeout: ms mode: mode from: nil
]

{ #category : 'runtime' }
StLiveCommands >> cmdStatus: args [
	^ self ok: (self m: {
		'epoch' -> server epoch.
		'pharo' -> SystemVersion current version.
		'image' -> Smalltalk imageFile fullName.
		'uptime_seconds' -> (DateAndTime now - server startedAt) asSeconds.
		'suspended_sessions' -> (server sessions values count: [ :s | s isSuspended ]).
		'sessions' -> server sessions size.
		'object_refs' -> server objectCount.
		'changes' -> server changes size })
]

{ #category : 'runtime' }
StLiveCommands >> cmdPing: args [
	^ self ok: (self m: { 'pong' -> true })
]

{ #category : 'runtime' }
StLiveCommands >> cmdImageInfo: args [
	| packages |
	packages := (PackageOrganizer default packages reject: [ :p | server isSystemPackage: p name ]) asSortedCollection: [ :a :b | a name <= b name ].
	^ self ok: (self m: {
		'epoch' -> server epoch.
		'pharo' -> SystemVersion current version.
		'image' -> Smalltalk imageFile fullName.
		'image_size_mb' -> (Smalltalk imageFile size // 1000000).
		'vm' -> (Smalltalk vm getSystemAttribute: 0).
		'loaded_packages' -> (packages asArray collect: [ :p | self m: { 'name' -> p name. 'classes' -> p definedClasses size } ]).
		'unsaved_changes' -> (server changes count: [ :c | (c at: 'saved' ifAbsent: [ true ]) not ]).
		'sessions_suspended' -> (server sessions values count: [ :x | x isSuspended ]) })
]

{ #category : 'runtime' }
StLiveCommands >> cmdLoadBaseline: args [
	"Load a Metacello baseline (in a job: failures become debug sessions) and report what changed."
	| name repository groups before timeout holder response |
	name := self need: 'name' in: args.
	repository := self need: 'repository' in: args.
	groups := (args at: 'groups' ifAbsent: [ #() ]) asArray.
	timeout := self int: 'timeout' in: args default: 600000.
	before := (PackageOrganizer default packages collect: [ :p | p name ]) asSet.
	response := self
		runBlock: [
			| loader added |
			loader := Metacello new baseline: name; repository: repository; yourself.
			groups isEmpty ifTrue: [ loader load ] ifFalse: [ loader load: groups ].
			added := ((PackageOrganizer default packages collect: [ :p | p name ]) asSet difference: before) asSortedCollection asArray.
			holder := self m: {
				'new_package_count' -> added size.
				'new_packages' -> (added first: (15 min: added size)).
				'new_packages_truncated' -> (added size > 15) }.
			true ]
		origin: (self originCmd: 'load.baseline' args: args)
		timeout: timeout
		mode: 'first'.
	((response at: 'ok') and: [ holder notNil ]) ifTrue: [
		| result |
		result := response at: 'result'.
		result removeKey: 'value'.
		result at: 'loaded' put: holder ].
	^ response
]

{ #category : 'runtime' }
StLiveCommands >> cmdImageSave: args [
	Smalltalk snapshot: true andQuit: false.
	^ self ok: (self m: { 'saved' -> true })
]

{ #category : 'runtime' }
StLiveCommands >> cmdImageStop: args [
	"Destructive: requires force: true."
	(args at: 'force' ifAbsent: [ false ]) == true
		ifFalse: [ self fail: 'confirmation_required' message: 'Stopping the image discards all unsaved state and debug sessions; pass force.' ].
	[ (Delay forMilliseconds: 300) wait. Smalltalk quitPrimitive ] forkAt: Processor userInterruptPriority.
	^ self ok: (self m: { 'stopping' -> true })
]

{ #category : 'eval' }
StLiveCommands >> cmdEval: args [
	| code receiver method timeout |
	code := self need: 'code' in: args.
	receiver := (args at: 'in' ifAbsent: [ nil ]) ifNil: [ nil ] ifNotNil: [ :r | server objectFor: r ].
	timeout := self int: 'timeout' in: args default: 30000.
	method := self compileDoit: code receiver: receiver context: nil.
	^ self
		runBlock: [ method valueWithReceiver: receiver arguments: #() ]
		origin: (self originCmd: 'eval' args: args)
		timeout: timeout
		mode: 'first'
]

{ #category : 'objects' }
StLiveCommands >> cmdObjShow: args [
	| obj |
	obj := server objectFor: (self need: 'ref' in: args).
	^ self ok: (server describer
		detail: obj
		offset: (self int: 'offset' in: args default: 0)
		limit: (self int: 'limit' in: args default: 20)
		raw: (args at: 'raw' ifAbsent: [ false ]) == true)
]

{ #category : 'objects' }
StLiveCommands >> cmdObjItems: args [
	| obj |
	obj := server objectFor: (self need: 'ref' in: args).
	^ self ok: (server describer
		items: obj
		offset: (self int: 'offset' in: args default: 0)
		limit: (self int: 'limit' in: args default: 20))
]

{ #category : 'objects' }
StLiveCommands >> cmdObjText: args [
	"Full text of a string-like object (or the printString of any object), paginated by characters."
	| obj text offset limit |
	obj := server objectFor: (self need: 'ref' in: args).
	text := obj isString ifTrue: [ obj asString ] ifFalse: [ obj printString ].
	offset := self int: 'offset' in: args default: 0.
	limit := self int: 'limit' in: args default: 2000.
	^ self ok: (self m: {
		'total' -> text size.
		'offset' -> offset.
		'text' -> (text copyFrom: (offset + 1 min: text size + 1) to: (offset + limit min: text size)).
		'truncated' -> (offset + limit < text size) })
]

{ #category : 'objects' }
StLiveCommands >> cmdObjVar: args [
	| obj name idx value |
	obj := server objectFor: (self need: 'ref' in: args).
	name := (self need: 'name' in: args) asString.
	idx := obj class allInstVarNames indexOf: name.
	idx = 0 ifTrue: [ self fail: 'unknown_variable' message: obj class printString , ' has no instance variable ' , name ].
	value := obj instVarAt: idx.
	^ self ok: (self m: { 'name' -> name. 'value' -> (self summary: value) })
]

{ #category : 'objects' }
StLiveCommands >> cmdObjCompare: args [
	| a b |
	a := server objectFor: (self need: 'a' in: args).
	b := server objectFor: (self need: 'b' in: args).
	^ self ok: (self m: {
		'identical' -> (a == b).
		'equal' -> ([ a = b ] on: Error do: [ :e | nil ]).
		'same_class' -> (a class == b class).
		'a' -> (self summary: a).
		'b' -> (self summary: b) })
]

{ #category : 'objects' }
StLiveCommands >> cmdObjRelease: args [
	(args at: 'all' ifAbsent: [ false ]) == true
		ifTrue: [ ^ self ok: (self m: { 'released' -> server clearObjects }) ].
	server releaseObject: (self need: 'ref' in: args).
	^ self ok: (self m: { 'released' -> 1 })
]

{ #category : 'code' }
StLiveCommands >> name: aName matches: aPattern [
	"Substring match, or glob match when the pattern contains * or #."
	^ (aPattern includes: $*) | (aPattern includes: $#)
		ifTrue: [ aPattern match: aName ]
		ifFalse: [ aName includesSubstring: aPattern ]
]

{ #category : 'code' }
StLiveCommands >> cmdFindClass: args [
	| pattern limit matches |
	pattern := (self need: 'pattern' in: args) asLowercase.
	limit := self int: 'limit' in: args default: 20.
	matches := (Smalltalk globals allClasses select: [ :c | self name: c name asLowercase matches: pattern ])
		asSortedCollection: [ :a :b | (a name asLowercase = pattern) & (b name asLowercase ~= pattern) or: [ a name size < b name size or: [ a name size = b name size and: [ a name <= b name ] ] ] ].
	^ self ok: (self m: {
		'total' -> matches size.
		'truncated' -> (matches size > limit).
		'classes' -> ((matches asArray first: (limit min: matches size)) collect: [ :c |
			self m: { 'name' -> c name. 'package' -> c package name. 'superclass' -> (c superclass ifNotNil: [ :s | s name ]) } ]) })
]

{ #category : 'code' }
StLiveCommands >> cmdFindPackage: args [
	| pattern limit matches |
	pattern := (self need: 'pattern' in: args) asLowercase.
	limit := self int: 'limit' in: args default: 30.
	matches := (PackageOrganizer default packages select: [ :p | self name: p name asLowercase matches: pattern ]) asSortedCollection: [ :a :b | a name <= b name ].
	^ self ok: (self m: {
		'total' -> matches size.
		'truncated' -> (matches size > limit).
		'packages' -> ((matches asArray first: (limit min: matches size)) collect: [ :p | self m: { 'name' -> p name. 'classes' -> p definedClasses size } ]) })
]

{ #category : 'code' }
StLiveCommands >> methodRef: aMethod [
	^ aMethod methodClass printString , '>>#' , aMethod selector
]

{ #category : 'code' }
StLiveCommands >> cmdFindImplementors: args [
	| sel limit found |
	sel := (self need: 'selector' in: args) asSymbol.
	limit := self int: 'limit' in: args default: 30.
	found := OrderedCollection new.
	self allBehaviorsDo: [ :b | (b includesSelector: sel) ifTrue: [ found add: b >> sel ] ].
	^ self ok: (self m: {
		'total' -> found size.
		'truncated' -> (found size > limit).
		'implementors' -> ((found asArray first: (limit min: found size)) collect: [ :cm | self methodRef: cm ]) })
]

{ #category : 'code' }
StLiveCommands >> cmdFindSenders: args [
	| sel limit found |
	sel := (self need: 'selector' in: args) asSymbol.
	limit := self int: 'limit' in: args default: 30.
	found := OrderedCollection new.
	self allBehaviorsDo: [ :b |
		b methodsDo: [ :cm | (cm sendsSelector: sel) ifTrue: [ found add: cm ] ] ].
	^ self ok: (self m: {
		'total' -> found size.
		'truncated' -> (found size > limit).
		'senders' -> ((found asArray first: (limit min: found size)) collect: [ :cm | self methodRef: cm ]) })
]

{ #category : 'code' }
StLiveCommands >> cmdClassShow: args [
	| cls offset limit selectors sorted |
	cls := self classNamed: (self need: 'name' in: args).
	offset := self int: 'offset' in: args default: 0.
	limit := self int: 'limit' in: args default: 60.
	sorted := cls selectors asSortedCollection asArray.
	^ self ok: (self m: {
		'name' -> cls printString.
		'package' -> ([ cls package name ] on: Error do: [ :e | nil ]).
		'superclasses' -> (cls allSuperclasses collect: [ :c | c printString ]).
		'subclasses' -> ((cls subclasses first: (30 min: cls subclasses size)) collect: [ :c | c printString ]).
		'instance_variables' -> cls instVarNames asArray.
		'class_variables' -> cls classVarNames asArray.
		'comment' -> ([ cls comment asString ] on: Error do: [ :e | '' ]) .
		'method_count' -> sorted size.
		'methods' -> ((sorted copyFrom: (offset + 1 min: sorted size + 1) to: (offset + limit min: sorted size)) collect: [ :s |
			self m: { 'selector' -> s. 'protocol' -> ([ (cls >> s) protocol name ] on: Error do: [ :e | nil ]) } ]).
		'truncated' -> (offset + limit < sorted size) })
]

{ #category : 'code' }
StLiveCommands >> cmdMethodShow: args [
	| target parts cls cm |
	target := self need: 'target' in: args.
	parts := target splitOn: '>>'.
	parts size = 2 ifFalse: [ self fail: 'bad_argument' message: 'Expected Class>>selector (or Class class>>selector)' ].
	cls := self classNamed: parts first.
	cm := cls compiledMethodAt: (parts second trimBoth copyWithout: $#) asSymbol ifAbsent: [ self fail: 'unknown_method' message: 'No method ' , target ].
	^ self ok: (self m: {
		'class' -> cls printString.
		'selector' -> cm selector.
		'protocol' -> ([ cm protocol name ] on: Error do: [ :e | nil ]).
		'package' -> ([ cm package name ] on: Error do: [ :e | nil ]).
		'author' -> ([ cm author ] on: Error do: [ :e | nil ]).
		'source' -> (self lf: cm sourceCode) })
]

{ #category : 'code' }
StLiveCommands >> cmdClassCreate: args [
	| name sup ivars package builder cls |
	name := self need: 'name' in: args.
	sup := self classNamed: (args at: 'superclass' ifAbsent: [ 'Object' ]).
	ivars := ((args at: 'ivars' ifAbsent: [ '' ]) substrings: ' ,') collect: [ :s | s asSymbol ].
	package := args at: 'package' ifAbsent: [ 'Live-Scratch' ].
	cls := (sup << name asSymbol)
		slots: ivars;
		package: package;
		install.
	server changes add: (self m: {
		'time' -> DateAndTime now printString.
		'kind' -> 'class-create'.
		'saved' -> false.
		'class' -> cls name.
		'class_name' -> cls name.
		'package' -> package.
		'class_package' -> package.
		'superclass' -> sup name.
		'instance_variables' -> cls instVarNames asArray }).
	^ self ok: (self m: { 'class' -> cls name. 'package' -> package. 'instance_variables' -> cls instVarNames asArray })
]

{ #category : 'code' }
StLiveCommands >> staleFramesFor: oldMethod [
	"Frames in suspended sessions that still run the given (now replaced) method."
	| found |
	found := OrderedCollection new.
	server sessions valuesDo: [ :s |
		s isSuspended ifTrue: [
			(s contextsFrom: 0 limit: 400) key withIndexDo: [ :ctx :i |
				(ctx home ifNotNil: [ :h | h method ]) == oldMethod ifTrue: [
					found add: (self m: { 'session' -> s id. 'frame' -> (i - 1) }) ] ] ] ].
	^ found asArray
]

{ #category : 'code' }
StLiveCommands >> cmdMethodCompile: args [
	| cls source protocol old new selector entry |
	cls := self classNamed: (self need: 'class' in: args).
	source := self lf: (self need: 'source' in: args) trimRight.
	selector := [ (OpalCompiler new source: source; class: cls; parse) selector ]
		on: OCCodeError do: [ :e | ^ (StLiveFailure code: 'syntax_error' message: e messageText asString details: (self m: { 'position' -> e location })) signal ].
	old := cls compiledMethodAt: selector ifAbsent: [ nil ].
	protocol := args at: 'protocol' ifAbsent: [ old ifNil: [ 'as yet unclassified' ] ifNotNil: [ :o | o protocol name ] ].
	new := [ cls compile: source classified: protocol ]
		on: OCCodeError, OCUndeclaredVariableWarning
		do: [ :e |
			^ (StLiveFailure code: 'compile_error' message: e messageText asString details: (self m: { 'position' -> ([ e location ] on: Error do: [ :x | nil ]) })) signal ].
	new isNil ifTrue: [ self fail: 'compile_error' message: 'Compilation failed' ].
	entry := self m: {
		'time' -> DateAndTime now printString.
		'kind' -> 'method-compile'.
		'saved' -> false.
		'package' -> ([ (cls compiledMethodAt: selector) package name ] on: Error do: [ :e | cls instanceSide package name ]).
		'class_package' -> ([ cls instanceSide package name ] on: Error do: [ :e | nil ]).
		'class_name' -> cls instanceSide name.
		'side' -> (cls isMeta ifTrue: [ 'class' ] ifFalse: [ 'instance' ]).
		'class' -> cls printString.
		'selector' -> selector.
		'protocol' -> protocol.
		'old_source' -> (old ifNotNil: [ :o | self lf: o sourceCode ]).
		'new_source' -> source }.
	server changes add: entry.
	^ self ok: (self m: {
		'class' -> cls printString.
		'selector' -> selector.
		'created' -> old isNil.
		'unchanged' -> (old notNil and: [ (self lf: old sourceCode) trimRight = source ]).
		'protocol' -> protocol.
		'note' -> ((old isNil and: [ (args includesKey: 'protocol') not ]) ifTrue: [ 'New method without --protocol: filed under as yet unclassified. Give --protocol (or *Package for an extension) before save.' ]).
		'change_index' -> server changes size.
		'stale_frames' -> (old ifNil: [ #() ] ifNotNil: [ :o | self staleFramesFor: o ]) })
]

{ #category : 'code' }
StLiveCommands >> cmdMethodRemove: args [
	| cls sel |
	(args at: 'force' ifAbsent: [ false ]) == true
		ifFalse: [ self fail: 'confirmation_required' message: 'Removing a method is destructive; pass force.' ].
	cls := self classNamed: (self need: 'class' in: args).
	sel := (self need: 'selector' in: args) asSymbol.
	(cls includesSelector: sel) ifFalse: [ self fail: 'unknown_method' message: 'No method ' , cls printString , '>>#' , sel ].
	server changes add: (self m: {
		'time' -> DateAndTime now printString.
		'kind' -> 'method-remove'.
		'saved' -> false.
		'package' -> ([ (cls >> sel) package name ] on: Error do: [ :e | nil ]).
		'class_package' -> ([ cls instanceSide package name ] on: Error do: [ :e | nil ]).
		'class_name' -> cls instanceSide name.
		'side' -> (cls isMeta ifTrue: [ 'class' ] ifFalse: [ 'instance' ]).
		'class' -> cls printString.
		'selector' -> sel.
		'old_source' -> (self lf: (cls >> sel) sourceCode) }).
	cls removeSelector: sel.
	^ self ok: (self m: { 'removed' -> true })
]

{ #category : 'code' }
StLiveCommands >> unsavedByPackage [
	| counts |
	counts := OrderedDictionary new.
	server changes do: [ :c |
		(c at: 'saved' ifAbsent: [ true ]) ifFalse: [
			| pkg |
			pkg := (c at: 'package' ifAbsent: [ nil ]) ifNil: [ '?' ].
			counts at: pkg put: (counts at: pkg ifAbsent: [ 0 ]) + 1 ] ].
	^ counts
]

{ #category : 'code' }
StLiveCommands >> cmdChangesList: args [
	| offset limit all |
	offset := self int: 'offset' in: args default: 0.
	limit := self int: 'limit' in: args default: 20.
	all := server changes asArray.
	^ self ok: (self m: {
		'total' -> all size.
		'unsaved_by_package' -> (self unsavedByPackage).
		'changes' -> ((all copyFrom: (offset + 1 min: all size + 1) to: (offset + limit min: all size)) withIndexCollect: [ :c :i |
			self m: { 'index' -> (offset + i). 'time' -> (c at: 'time'). 'kind' -> (c at: 'kind'). 'class' -> (c at: 'class' ifAbsent: [ nil ]). 'selector' -> (c at: 'selector' ifAbsent: [ nil ]) } ]) })
]

{ #category : 'code' }
StLiveCommands >> cmdChangesPending: args [
	"Unsaved changes (optionally of one package), in order, with everything needed to write them to Tonel."
	| package list |
	package := args at: 'package' ifAbsent: [ nil ].
	list := OrderedCollection new.
	server changes withIndexDo: [ :c :i |
		((c at: 'saved' ifAbsent: [ true ]) not and: [ package isNil or: [ (c at: 'package' ifAbsent: [ nil ]) = package ] ])
			ifTrue: [ list add: ((c copy) at: 'index' put: i; yourself) ] ].
	^ self ok: (self m: { 'changes' -> list asArray })
]

{ #category : 'code' }
StLiveCommands >> cmdChangesMarkSaved: args [
	| indexes |
	indexes := self need: 'indexes' in: args.
	indexes do: [ :i | (server changes at: i) at: 'saved' put: true ].
	^ self ok: (self m: { 'marked' -> indexes size })
]

{ #category : 'code' }
StLiveCommands >> cmdChangesShow: args [
	| i |
	i := self int: 'index' in: args default: 1.
	(i between: 1 and: server changes size) ifFalse: [ self fail: 'unknown_change' message: 'No change #' , i printString ].
	^ self ok: (server changes at: i)
]

{ #category : 'tests' }
StLiveCommands >> testsFor: target [
	"Answer an array of { testClass. selector } pairs."
	| parts cls pkg classes |
	(target includesSubstring: '>>') ifTrue: [
		parts := target splitOn: '>>'.
		cls := self classNamed: parts first.
		^ { { cls. (parts second trimBoth copyWithout: $#) asSymbol } } ].
	cls := Smalltalk globals at: target asSymbol ifAbsent: [ nil ].
	cls isBehavior ifTrue: [
		(cls inheritsFrom: TestCase) ifFalse: [ self fail: 'not_a_test' message: target , ' is not a TestCase' ].
		cls testSelectors isEmpty ifTrue: [ self fail: 'no_tests' message: cls name , ' has no test methods (abstract or only inherited helpers)' ].
		^ cls testSelectors collect: [ :s | { cls. s } ] ].
	pkg := PackageOrganizer default packageNamed: target ifAbsent: [ nil ].
	classes := pkg
		ifNotNil: [ pkg definedClasses ]
		ifNil: [ Smalltalk globals allClasses select: [ :c | [ c package name match: target ] on: Error do: [ :e | false ] ] ].
	classes := classes select: [ :c | c isTestCase and: [ c isAbstract not ] ].
	classes isEmpty ifTrue: [
		| similar |
		similar := (Smalltalk globals allClasses select: [ :c | c isTestCase and: [ c isAbstract not and: [ (c name asLowercase includesSubstring: target asLowercase) ] ] ]) collect: [ :c | c name ].
		self fail: 'no_tests' message: 'No test class, test or package matches ' , target , (similar isEmpty ifTrue: [ ' (see: stlive test list <pattern>)' ] ifFalse: [ '. Similar test classes: ' , (similar first: (8 min: similar size)) asArray printString ]) ].
	^ classes asSortedCollection: [ :a :b | a name <= b name ]; flatCollect: [ :c | c testSelectors collect: [ :s | { c. s } ] ]
]

{ #category : 'tests' }
StLiveCommands >> terminateOldTestSessions [
	server sessions valuesDo: [ :s |
		(s isSuspended and: [ (s origin at: 'cmd') = 'test.run' ]) ifTrue: [ server terminateSession: s ] ]
]

{ #category : 'tests' }
StLiveCommands >> failureRecord: err test: testId [
	"One failing test as a dictionary used for grouping."
	| session frames first message sig |
	session := (err at: 'session' ifAbsent: [ nil ]) ifNotNil: [ :id | server sessions at: id ifAbsent: [ nil ] ].
	frames := session
		ifNil: [ #() ]
		ifNotNil: [ ((session contextsFrom: 0 limit: 3) key collect: [ :c | server describer labelOf: c ]) ].
	first := session ifNotNil: [ (self firstApplicationFrame: session) ].
	message := err at: 'message'.
	sig := (err at: 'type') , '|' , (message copyWithRegex: '[0-9]+' matchesReplacedWith: '#') , '|' , (first ifNil: [ '' ] ifNotNil: [ :i | server describer labelOf: (session contextAt: i) ]).
	^ self m: {
		'test' -> testId.
		'status' -> (((err at: 'exception' ifAbsent: [ nil ]) ifNotNil: [ :e | e at: 'class' ]) = 'TestFailure'
			ifTrue: [ 'failure' ] ifFalse: [ (err at: 'kind') = 'timeout' ifTrue: [ 'timeout' ] ifFalse: [ 'error' ] ]).
		'type' -> (err at: 'type').
		'message' -> message.
		'session' -> (err at: 'session' ifAbsent: [ nil ]).
		'signature' -> sig.
		'top_frames' -> frames.
		'first_application_frame' -> (first ifNotNil: [ :i | self m: { 'index' -> i. 'label' -> (server describer labelOf: (session contextAt: i)). 'statement' -> (server describer statementOf: (session contextAt: i)) } ]) }
]

{ #category : 'tests' }
StLiveCommands >> groupFailures: records keep: keep discard: discard [
	"Group by signature (same exception type, message modulo digits, same application frame)."
	| groups ordered |
	groups := OrderedDictionary new.
	records do: [ :r | (groups at: (r at: 'signature') ifAbsentPut: [ OrderedCollection new ]) add: r ].
	ordered := groups values asSortedCollection: [ :x :y | x size > y size ].
	^ ordered asArray withIndexCollect: [ :members :i |
		| first sessions kept |
		first := members first.
		sessions := members collect: [ :r | r at: 'session' ] thenSelect: #notNil.
		kept := OrderedCollection new.
		sessions withIndexDo: [ :id :n |
			(discard not and: [ keep or: [ n = 1 ] ])
				ifTrue: [ kept add: id ]
				ifFalse: [ server terminateSession: (server sessionFor: id) ] ].
		self m: {
			'group' -> i.
			'count' -> members size.
			'status' -> (first at: 'status').
			'type' -> (first at: 'type').
			'message' -> (first at: 'message').
			'first_application_frame' -> (first at: 'first_application_frame').
			'top_frames' -> (first at: 'top_frames').
			'tests' -> ((members first: (30 min: members size)) collect: [ :r | r at: 'test' ]).
			'tests_truncated' -> (members size > 30).
			'sessions' -> kept asArray.
			'sessions_ended' -> (sessions size - kept size) } ]
]

{ #category : 'tests' }
StLiveCommands >> cmdTestList: args [
	"Test classes (not abstract) whose class or package name matches the glob/substring pattern."
	| pattern limit found |
	pattern := ((args at: 'pattern' ifAbsent: [ '' ]) ifEmpty: [ '*' ]) asLowercase.
	limit := self int: 'limit' in: args default: 50.
	found := (Smalltalk globals allClasses select: [ :c |
		c isTestCase and: [ c isAbstract not and: [
			(self name: c name asLowercase matches: pattern) or: [ self name: ([ c package name ] on: Error do: [ :e | '' ]) asLowercase matches: pattern ] ] ] ])
		asSortedCollection: [ :a :b | a name <= b name ].
	^ self ok: (self m: {
		'total' -> found size.
		'truncated' -> (found size > limit).
		'test_classes' -> ((found asArray first: (limit min: found size)) collect: [ :c |
			self m: { 'name' -> c name. 'package' -> ([ c package name ] on: Error do: [ :e | nil ]). 'tests' -> c testSelectors size } ]) })
]

{ #category : 'tests' }
StLiveCommands >> cmdTestRun: args [
	| target tests timeout passed records started keep discard groups |
	target := self need: 'target' in: args.
	timeout := self int: 'timeout' in: args default: 10000.
	keep := (args at: 'keep_sessions' ifAbsent: [ false ]) == true.
	discard := (args at: 'discard_sessions' ifAbsent: [ false ]) == true.
	keep ifFalse: [ self terminateOldTestSessions ].
	tests := self testsFor: target.
	passed := 0.
	records := OrderedCollection new.
	started := Time millisecondClockValue.
	tests do: [ :pair |
		| cls sel response testId |
		cls := pair first.
		sel := pair second.
		testId := cls name , '>>' , sel.
		response := self
			runBlock: [ (cls selector: sel) runCase ]
			origin: (self originCmd: 'test.run' args: (self m: { 'target' -> testId. 'timeout' -> timeout }))
			timeout: timeout
			mode: 'first'.
		(response at: 'ok')
			ifTrue: [ passed := passed + 1 ]
			ifFalse: [ records add: (self failureRecord: (response at: 'error') test: testId) ] ].
	groups := self groupFailures: records keep: keep discard: discard.
	^ self ok: (self m: {
		'target' -> target.
		'ran' -> tests size.
		'passed' -> passed.
		'failed' -> records size.
		'duration_ms' -> (Time millisecondClockValue - started).
		'groups' -> groups })
]

{ #category : 'debug' }
StLiveCommands >> session: args [
	^ server sessionFor: (self need: 'session' in: args)
]

{ #category : 'debug' }
StLiveCommands >> frameContext: aSession args: args [
	| index ctx |
	index := self int: 'frame' in: args default: 0.
	ctx := aSession contextAt: index.
	ctx ifNil: [ self fail: 'unknown_frame' message: 'Session ' , aSession id , ' has no frame ' , index printString , ' (frames: ' , aSession frameCount printString , ')' ].
	^ ctx
]

{ #category : 'debug' }
StLiveCommands >> requireSuspended: aSession [
	aSession isSuspended ifFalse: [
		self fail: 'session_not_suspended' message: 'Session ' , aSession id , ' is ' , aSession state , '; its frames are no longer available' ].
	aSession awaitParked ifFalse: [ self fail: 'session_not_parked' message: 'Session ' , aSession id , ' is not suspended in the VM' ]
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugList: args [
	| all |
	all := (args at: 'all' ifAbsent: [ false ]) == true.
	^ self ok: (self m: { 'sessions' -> ((server sessions values select: [ :s | all or: [ s state ~~ #terminated ] ]) collect: [ :s |
		self m: {
			'session' -> s id.
			'state' -> s state.
			'type' -> s typeName.
			'message' -> (s messageText size > 100 ifTrue: [ (s messageText first: 100) , '...' ] ifFalse: [ s messageText ]).
			'age_seconds' -> (DateAndTime now - s createdAt) asSeconds } ]) asArray })
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugInspect: args [
	| s |
	s := self session: args.
	^ self ok: (self m: {
		'session' -> s id.
		'state' -> s state.
		'kind' -> s kind.
		'type' -> s typeName.
		'message' -> s messageText.
		'exception' -> (s exception ifNotNil: [ :e | self summary: e ]).
		'origin' -> s origin.
		'frame_count' -> (s isSuspended ifTrue: [ s frameCount ] ifFalse: [ nil ]).
		'frame_count_is_lower_bound' -> (s isSuspended and: [ s frameCount >= StLiveSession maxFrames ]).
		'top' -> (s topContext ifNotNil: [ :c | server describer frame: c index: 0 ]).
		'first_application_frame' -> (s isSuspended ifTrue: [ self firstApplicationFrame: s ] ifFalse: [ nil ]).
		'recursion' -> (s isSuspended ifTrue: [ self recursionOf: s ] ifFalse: [ nil ]).
		'continued_from' -> s continuedFrom })
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugFrames: args [
	| s offset limit page |
	s := self session: args.
	self requireSuspended: s.
	offset := self int: 'offset' in: args default: 0.
	limit := self int: 'limit' in: args default: 5.
	page := s contextsFrom: offset limit: limit.
	^ self ok: (self m: {
		'session' -> s id.
		'total' -> page value.
		'total_is_lower_bound' -> (page value >= StLiveSession maxFrames).
		'offset' -> offset.
		'frames' -> (page key withIndexCollect: [ :ctx :i | server describer frame: ctx index: offset + i - 1 ]).
		'truncated' -> (offset + limit < page value) })
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugFrame: args [
	| s index |
	s := self session: args.
	self requireSuspended: s.
	index := self int: 'frame' in: args default: 0.
	^ self ok: (server describer
		frameDetail: (self frameContext: s args: args)
		index: index
		session: s
		withSource: (args at: 'no_source' ifAbsent: [ false ]) ~~ true)
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugLocals: args [
	| s ctx |
	s := self session: args.
	self requireSuspended: s.
	ctx := self frameContext: s args: args.
	^ self ok: (self m: { 'frame' -> (self int: 'frame' in: args default: 0). 'variables' -> (server describer variables: ctx) })
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugReceiver: args [
	| s ctx |
	s := self session: args.
	self requireSuspended: s.
	ctx := self frameContext: s args: args.
	^ self ok: (server describer
		detail: ctx receiver
		offset: (self int: 'offset' in: args default: 0)
		limit: (self int: 'limit' in: args default: 20)
		raw: false)
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugEval: args [
	"Evaluate an expression in the context of a frame (temporaries and arguments are visible)."
	| s ctx code compiler |
	s := self session: args.
	self requireSuspended: s.
	ctx := self frameContext: s args: args.
	code := self need: 'code' in: args.
	[ self compileDoit: code receiver: ctx receiver context: ctx ]
		on: StLiveFailure do: [ :f |
			f messageText: f messageText , ' (evaluated in frame ' , (self int: 'frame' in: args default: 0) printString , ' of ' , s id , '; pick the frame that declares the variable with --frame N)'.
			f pass ].
	compiler := OpalCompiler new source: code; receiver: ctx receiver; context: ctx; yourself.
	^ self
		runBlock: [ compiler evaluate ]
		origin: (self originCmd: 'debug.eval' args: args)
		timeout: (self int: 'timeout' in: args default: 30000)
		mode: 'first'
]

{ #category : 'debug' }
StLiveCommands >> valueFor: args in: aSession [
	"Optional 'value' argument: a Smalltalk expression evaluated in the top frame."
	| code ctx |
	code := args at: 'value' ifAbsent: [ ^ nil ].
	ctx := aSession topContext.
	^ (OpalCompiler new source: code; receiver: ctx receiver; context: ctx; yourself) evaluate
]

{ #category : 'debug' }
StLiveCommands >> continueSession: aSession mode: mode args: args action: aBlock [
	| job timeout |
	job := aSession job.
	timeout := self int: 'timeout' in: args default: 30000.
	job markRunning.
	aSession state: #resumed.
	aBlock value.
	job process resume.
	^ self awaitJob: job timeout: timeout mode: mode from: aSession
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugResume: args [
	"Continue the suspended execution where it stopped. Does NOT re-run anything."
	| s |
	s := self session: args.
	self requireSuspended: s.
	^ self continueSession: s mode: 'resume' args: args action: [ s resumeValue: (self valueFor: args in: s) ]
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugRestart: args [
	"Unwind to the given frame and restart that method with the current code."
	| s ctx |
	s := self session: args.
	self requireSuspended: s.
	ctx := self frameContext: s args: args.
	^ self continueSession: s mode: 'restart' args: args action: [
		| home new |
		home := ctx home ifNil: [ ctx ].
		new := [ home methodClass compiledMethodAt: home selector ifAbsent: [ nil ] ] on: Error do: [ :e | nil ].
		(new notNil and: [ new ~~ home method ])
			ifTrue: [ s debugSession rewindContextToMethod: new fromContext: home ]
			ifFalse: [ s debugSession restart: ctx ] ]
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugReturn: args [
	"Make the given frame return a value, then continue."
	| s ctx value |
	s := self session: args.
	self requireSuspended: s.
	ctx := self frameContext: s args: args.
	value := (OpalCompiler new source: (self need: 'value' in: args); receiver: ctx receiver; context: ctx; yourself) evaluate.
	^ self continueSession: s mode: 'return' args: args action: [ s debugSession returnValue: value from: ctx ]
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugRerun: args [
	"Execute the original logical operation again, from scratch, with the code now in the image."
	| s origin response |
	s := self session: args.
	origin := s origin.
	response := self dispatch: (self m: { 'cmd' -> (origin at: 'cmd'). 'args' -> (origin at: 'args') }).
	response at: 'rerun_of' put: s id.
	response at: 'mode' put: 'rerun'.
	(response at: 'ok') ifTrue: [
		(response at: 'result') at: 'mode' put: 'rerun'.
		self annotate: (response at: 'result') mode: 'rerun' ].
	^ response
]

{ #category : 'debug' }
StLiveCommands >> cmdDebugTerminate: args [
	| s n |
	(args at: 'all' ifAbsent: [ false ]) == true ifTrue: [
		n := 0.
		server sessions valuesDo: [ :each | each isSuspended ifTrue: [ server terminateSession: each. n := n + 1 ] ].
		^ self ok: (self m: { 'terminated' -> n }) ].
	s := self session: args.
	server terminateSession: s.
	^ self ok: (self m: { 'terminated' -> 1. 'session' -> s id })
]
