"
Turns objects, contexts and frames into compact JSON-ready dictionaries.
Non-trivial objects are represented by a summary plus a reference (obj-...) for later navigation.
"
Class {
	#name : 'StLiveDescriber',
	#superclass : 'Object',
	#instVars : [
		'server'
	],
	#package : 'StLive'
}

{ #category : 'instance creation' }
StLiveDescriber class >> on: aServer [
	^ self new server: aServer; yourself
]

{ #category : 'accessing' }
StLiveDescriber >> server: aServer [
	server := aServer
]

{ #category : 'objects' }
StLiveDescriber >> print: anObject limit: n [
	^ [ anObject printStringLimitedTo: n ] on: Error do: [ :e | '<printString failed: ' , e class name , '>' ]
]

{ #category : 'objects' }
StLiveDescriber >> isInline: obj [
	^ obj isNil or: [ (obj == true or: [ obj == false ]) or: [ obj isNumber or: [ obj isCharacter or: [ obj isString and: [ obj size <= 100 ] ] ] ] ]
]

{ #category : 'objects' }
StLiveDescriber >> summary: obj [
	| d p |
	d := OrderedDictionary new.
	d at: 'class' put: obj class printString.
	p := self print: obj limit: 120.
	d at: 'print' put: p.
	(p size > 120 or: [ p endsWith: '[..]' ]) ifTrue: [
		d at: 'print_truncated' put: true.
		d at: 'hint' put: ((self isInline: obj) ifTrue: [ 'cut at 120 characters; eval --full gives the whole text' ] ifFalse: [ 'cut at 120 characters; `obj text <ref>` or eval --full gives the whole text' ]) ].
	(self isInline: obj) ifFalse: [ d at: 'ref' put: (server refFor: obj) ].
	^ d
]

{ #category : 'objects' }
StLiveDescriber >> isSystemClass: aClass [
	^ [ server isSystemPackage: aClass package name ] on: Error do: [ :e | false ]
]

{ #category : 'objects' }
StLiveDescriber >> detail: obj offset: offset limit: limit raw: raw [
	| d names showIvars slice ivars |
	d := self summary: obj.
	d at: 'ref' put: (server refFor: obj).
	d at: 'identity' put: obj identityHash.
	obj isCollection ifTrue: [ d at: 'size' put: ([ obj size ] on: Error do: [ :e | nil ]) ].
	obj isBehavior ifTrue: [
		d at: 'name' put: obj name.
		obj isClass ifTrue: [ d at: 'package' put: obj package name ] ].
	obj isBlock ifTrue: [
		d at: 'source' put: ([ obj sourceNode sourceCode ] on: Error do: [ :e | nil ]).
		d at: 'num_args' put: obj numArgs ].
	names := obj class allInstVarNames.
	showIvars := raw or: [ obj isCollection not or: [ (self isSystemClass: obj class) not ] ].
	showIvars
		ifTrue: [
			slice := names copyFrom: (offset + 1 min: names size + 1) to: (offset + limit min: names size).
			ivars := slice withIndexCollect: [ :name :i |
				| m v |
				v := obj instVarAt: offset + i.
				m := self summary: v.
				m at: 'name' put: name.
				m ].
			d at: 'instance_variables' put: ivars asArray.
			d at: 'instance_variable_count' put: names size.
			d at: 'truncated' put: offset + limit < names size ]
		ifFalse: [ d at: 'note' put: 'internal slots hidden for system collection; use items' ].
	(obj class isVariable and: [ obj isCollection not ]) ifTrue: [ d at: 'indexed_size' put: obj basicSize ].
	^ d
]

{ #category : 'objects' }
StLiveDescriber >> items: obj offset: offset limit: limit [
	| d total items array |
	d := OrderedDictionary new.
	(obj isDictionary)
		ifTrue: [
			array := obj associations.
			total := array size.
			items := ((array copyFrom: (offset + 1 min: total + 1) to: (offset + limit min: total)) collect: [ :assoc |
				OrderedDictionary new
					at: 'key' put: (self summary: assoc key);
					at: 'value' put: (self summary: assoc value);
					yourself ]) asArray ]
		ifFalse: [
			array := obj isCollection
				ifTrue: [ obj isSequenceable ifTrue: [ obj ] ifFalse: [ obj asArray ] ]
				ifFalse: [
					obj class isVariable
						ifTrue: [ (1 to: obj basicSize) collect: [ :i | obj basicAt: i ] ]
						ifFalse: [ ^ (StLiveFailure code: 'not_a_collection' message: 'Object has no elements: ' , (self print: obj limit: 60)) signal ] ].
			total := array size.
			items := ((offset + 1 to: (offset + limit min: total)) collect: [ :i |
				OrderedDictionary new
					at: 'index' put: i;
					at: 'value' put: (self summary: (array at: i));
					yourself ]) asArray ].
	d at: 'total' put: total.
	d at: 'offset' put: offset.
	d at: 'items' put: items.
	d at: 'truncated' put: offset + limit < total.
	offset + limit < total ifTrue: [ d at: 'next_offset' put: offset + limit ].
	^ d
]

{ #category : 'frames' }
StLiveDescriber >> originOf: aContext [
	"application | framework | tooling | eval"
	| home pkg |
	home := aContext home ifNil: [ aContext ].
	(home receiver isKindOf: StLiveJob) ifTrue: [ ^ 'tooling' ].
	home selector == #DoIt ifTrue: [ ^ 'eval' ].
	pkg := [ home method package name ] on: Error do: [ :e | nil ].
	pkg ifNil: [ ^ 'unknown' ].
	pkg = 'StLive' ifTrue: [ ^ 'tooling' ].
	^ (server isSystemPackage: pkg) ifTrue: [ 'framework' ] ifFalse: [ 'application' ]
]

{ #category : 'frames' }
StLiveDescriber >> labelOf: aContext [
	| home recvClass methodClass sel label |
	home := aContext home ifNil: [ aContext ].
	recvClass := aContext receiver class.
	methodClass := [ home methodClass ] on: Error do: [ :e | recvClass ].
	sel := [ home selector ] on: Error do: [ :e | '?' ].
	label := (recvClass == methodClass or: [ methodClass isNil ])
		ifTrue: [ recvClass printString , '>>#' , sel ]
		ifFalse: [ recvClass printString , '(' , methodClass printString , ')>>#' , sel ].
	^ aContext isBlockContext ifTrue: [ '[] in ' , label ] ifFalse: [ label ]
]

{ #category : 'frames' }
StLiveDescriber >> isStale: aContext [
	"True when the method this frame runs is no longer the installed version."
	| home cls |
	home := aContext home ifNil: [ ^ false ].
	home selector == #DoIt ifTrue: [ ^ false ].
	cls := [ home methodClass ] on: Error do: [ :e | ^ false ].
	cls ifNil: [ ^ false ].
	^ (cls compiledMethodAt: home selector ifAbsent: [ nil ]) ~~ home method
]

{ #category : 'frames' }
StLiveDescriber >> statementOf: aContext [
	^ [ | node |
		node := aContext method sourceNodeForPC: aContext pc.
		node statementNode sourceCode ] on: Error do: [ :e | nil ]
]

{ #category : 'frames' }
StLiveDescriber >> frame: aContext index: i [
	| d stmt |
	d := OrderedDictionary new.
	d at: 'index' put: i.
	d at: 'label' put: (self labelOf: aContext).
	d at: 'receiver_class' put: aContext receiver class printString.
	d at: 'origin' put: (self originOf: aContext).
	d at: 'is_block' put: aContext isBlockContext.
	d at: 'stale' put: (self isStale: aContext).
	stmt := self statementOf: aContext.
	stmt := stmt ifNotNil: [ (stmt copyReplaceAll: String crlf with: String lf) copyReplaceAll: String cr with: String lf ].
	stmt ifNotNil: [ d at: 'statement' put: (stmt size > 160 ifTrue: [ (stmt copyFrom: 1 to: 160) , '...' ] ifFalse: [ stmt ]) ].
	^ d
]

{ #category : 'frames' }
StLiveDescriber >> variableKinds: aContext [
	"Answer a Dictionary name -> { kind. scope }. Kinds: block_argument, block_temporary (own to a block),
	outer_block_argument/outer_block_temporary, argument/temporary (of the method)."
	| kinds node own |
	kinds := Dictionary new.
	node := [ aContext method sourceNode ] on: Error do: [ :e | nil ].
	own := true.
	[ node notNil ] whileTrue: [
		| prefix |
		prefix := node isBlock
			ifTrue: [ own ifTrue: [ 'block_' ] ifFalse: [ 'outer_block_' ] ]
			ifFalse: [ '' ].
		[ node arguments do: [ :a | kinds at: a name ifAbsentPut: [ { prefix , 'argument'. own ifTrue: [ 'own' ] ifFalse: [ 'enclosing' ] } ] ].
		node temporaries do: [ :t | kinds at: t name ifAbsentPut: [ { prefix , 'temporary'. own ifTrue: [ 'own' ] ifFalse: [ 'enclosing' ] } ] ] ] on: Error do: [ :e | nil ].
		node isBlock ifFalse: [ own := false ].
		node := node parent.
		own := false ].
	^ kinds
]

{ #category : 'frames' }
StLiveDescriber >> variables: aContext [
	"Array of { name, kind, scope } + value summary; the frame's own variables come first."
	| names kinds all |
	names := [ aContext tempNames ] on: Error do: [ :e | #() ].
	kinds := self variableKinds: aContext.
	all := names collect: [ :name |
		| m info |
		m := [ self summary: (aContext tempNamed: name) ] on: Error do: [ :e |
			OrderedDictionary new at: 'print' put: '<unavailable>'; yourself ].
		info := kinds at: name ifAbsent: [ { 'temporary'. 'own' } ].
		m at: 'name' put: name.
		m at: 'kind' put: info first.
		m at: 'scope' put: info last.
		m ].
	^ ((all select: [ :m | (m at: 'scope') = 'own' ]) , (all reject: [ :m | (m at: 'scope') = 'own' ])) asArray
]

{ #category : 'frames' }
StLiveDescriber >> frameDetail: aContext index: i session: aSession [
	^ self frameDetail: aContext index: i session: aSession withSource: true
]

{ #category : 'frames' }
StLiveDescriber >> frameDetail: aContext index: i session: aSession withSource: withSource [
	| d home src line interval |
	d := self frame: aContext index: i.
	home := aContext home ifNil: [ aContext ].
	d at: 'method_class' put: ([ home methodClass printString ] on: Error do: [ :e | nil ]).
	d at: 'selector' put: ([ home selector asString ] on: Error do: [ :e | nil ]).
	d at: 'package' put: ([ home method package name ] on: Error do: [ :e | nil ]).
	d at: 'receiver' put: (self summary: aContext receiver).
	d at: 'variables' put: (self variables: aContext).
	src := [ home method sourceCode ] on: Error do: [ :e | nil ].
	src := src ifNotNil: [ (src copyReplaceAll: String crlf with: String lf) copyReplaceAll: String cr with: String lf ].
	src ifNotNil: [
		interval := [ (aContext method sourceNodeForPC: aContext pc) statementNode sourceInterval ] on: Error do: [ :e | nil ].
		interval ifNotNil: [ d at: 'statement_line' put: (src copyFrom: 1 to: interval first) lines size ].
		withSource ifTrue: [ d at: 'source' put: (src size > 4000 ifTrue: [ (src copyFrom: 1 to: 4000) , '...' ] ifFalse: [ src ]).
		src size > 4000 ifTrue: [ d at: 'source_truncated' put: true ] ] ].
	d at: 'caller_index' put: ((aSession contextAt: i + 1) ifNil: [ nil ] ifNotNil: [ :c | i + 1 ]).
	d at: 'callee_index' put: (i > 0 ifTrue: [ i - 1 ] ifFalse: [ nil ]).
	^ d
]
