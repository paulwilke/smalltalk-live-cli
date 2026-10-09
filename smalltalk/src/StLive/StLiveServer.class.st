"
Line-oriented JSON server on 127.0.0.1. One request line in, one response line out, per connection.
Owns the object-reference registry, debug sessions and jobs of this image run.
"
Class {
	#name : 'StLiveServer',
	#superclass : 'Object',
	#instVars : [
		'epoch',
		'objects',
		'reverse',
		'nextObject',
		'sessions',
		'nextSession',
		'jobs',
		'nextJob',
		'listener',
		'running',
		'startedAt',
		'describer',
		'commands',
		'changes',
		'portFile'
	],
	#classVars : [
		'Current',
		'SystemPackages'
	],
	#package : 'StLive'
}

{ #category : 'accessing' }
StLiveServer class >> current [
	^ Current ifNil: [ Current := self new ]
]

{ #category : 'accessing' }
StLiveServer class >> recordSystemPackages [
	"Called once while preparing the image: packages that exist now count as framework code."
	SystemPackages := (PackageOrganizer default packages collect: [ :p | p name ]) asSet
]

{ #category : 'accessing' }
StLiveServer class >> systemPackages [
	^ SystemPackages ifNil: [ Set new ]
]

{ #category : 'starting' }
StLiveServer class >> startFromEnvironment [
	"STLIVE_PORTFILE: where to publish {port, epoch}. Without it the port is only printed."
	| env |
	env := Smalltalk os environment.
	^ self current startPortFile: (env at: 'STLIVE_PORTFILE' ifAbsent: [ nil ])
]

{ #category : 'instance creation' }
StLiveServer class >> newEpoch [
	| s |
	s := (((1 to: 4095) atRandom) printStringBase: 16) asLowercase.
	^ ('000' , s) last: 3
]

{ #category : 'initialization' }
StLiveServer >> initialize [
	super initialize.
	epoch := self class newEpoch.
	objects := OrderedDictionary new.
	reverse := IdentityDictionary new.
	nextObject := 0.
	sessions := OrderedDictionary new.
	nextSession := 0.
	jobs := Dictionary new.
	nextJob := 0.
	changes := OrderedCollection new.
	startedAt := DateAndTime now.
	running := false.
	describer := StLiveDescriber on: self.
	commands := StLiveCommands on: self
]

{ #category : 'accessing' }
StLiveServer >> epoch [
	^ epoch
]

{ #category : 'accessing' }
StLiveServer >> describer [
	^ describer
]

{ #category : 'accessing' }
StLiveServer >> startedAt [
	^ startedAt
]

{ #category : 'accessing' }
StLiveServer >> changes [
	^ changes
]

{ #category : 'accessing' }
StLiveServer >> sessions [
	^ sessions
]

{ #category : 'accessing' }
StLiveServer >> objectCount [
	^ objects size
]

{ #category : 'accessing' }
StLiveServer >> isSystemPackage: aName [
	^ self class systemPackages includes: aName
]

{ #category : 'references' }
StLiveServer >> refFor: anObject [
	^ reverse at: anObject ifAbsent: [
		| id |
		nextObject := nextObject + 1.
		id := 'obj-' , epoch , '-' , nextObject printString.
		objects at: id put: anObject.
		reverse at: anObject put: id.
		objects size > 5000 ifTrue: [ self dropOldestObjects: 1000 ].
		id ]
]

{ #category : 'references' }
StLiveServer >> dropOldestObjects: n [
	| keys |
	keys := objects keys first: n.
	keys do: [ :k | reverse removeKey: (objects at: k) ifAbsent: [ nil ]. objects removeKey: k ]
]

{ #category : 'references' }
StLiveServer >> objectsRegistry [
	^ objects
]

{ #category : 'references' }
StLiveServer >> reverseRegistry [
	^ reverse
]

{ #category : 'references' }
StLiveServer >> isRegistryInternal: anObject [
	"True for the internal arrays/associations of the registry dictionaries."
	^ [ | inner |
		inner := objects instVarNamed: 'dictionary'.
		anObject == inner
			or: [ anObject == (inner instVarNamed: 'array')
			or: [ anObject == (reverse instVarNamed: 'array')
			or: [ (anObject isKindOf: Association) and: [ ((reverse instVarNamed: 'array') identityIncludes: anObject) or: [ anObject key isString and: [ objects includesKey: anObject key ] ] ] ] ] ] ]
		on: Error do: [ :e | false ]
]

{ #category : 'references' }
StLiveServer >> clearObjects [
	| n |
	n := objects size.
	objects removeAll.
	reverse removeAll.
	^ n
]

{ #category : 'references' }
StLiveServer >> releaseObject: ref [
	reverse removeKey: (self objectFor: ref) ifAbsent: [ nil ].
	objects removeKey: ref
]

{ #category : 'references' }
StLiveServer >> checkRef: ref prefix: prefix [
	| parts |
	(ref isString and: [ (parts := ref substrings: '-') size = 3 and: [ parts first = prefix ] ])
		ifFalse: [ ^ (StLiveFailure code: 'invalid_ref' message: 'Not a valid ' , prefix , ' reference: ' , ref printString) signal ].
	parts second = epoch
		ifFalse: [ ^ (StLiveFailure code: 'stale_ref' message: 'Reference ' , ref , ' belongs to an earlier run of this image (current run: ' , epoch , ')') signal ]
]

{ #category : 'references' }
StLiveServer >> objectFor: ref [
	self checkRef: ref prefix: 'obj'.
	^ objects at: ref ifAbsent: [ (StLiveFailure code: 'unknown_ref' message: 'Object reference ' , ref , ' is unknown or was released/evicted') signal ]
]

{ #category : 'sessions' }
StLiveServer >> newJob: aBlock origin: aDictionary [
	| job |
	nextJob := nextJob + 1.
	job := StLiveJob server: self id: 'job-' , epoch , '-' , nextJob printString block: aBlock origin: aDictionary.
	jobs at: job id put: job.
	^ job
]

{ #category : 'sessions' }
StLiveServer >> newSessionFor: aJob exception: anException context: aContext kind: aKind [
	| s |
	nextSession := nextSession + 1.
	s := StLiveSession
		id: 'dbg-' , epoch , '-' , nextSession printString
		job: aJob
		exception: anException
		context: aContext
		kind: aKind.
	sessions at: s id put: s.
	s markSuspended.
	self trimSessions.
	^ s
]

{ #category : 'sessions' }
StLiveServer >> trimSessions [
	"Keep at most 60 sessions; the oldest ones are terminated and forgotten."
	[ sessions size > 60 ] whileTrue: [
		| s |
		s := sessions values first.
		self terminateSession: s.
		sessions removeKey: s id ]
]

{ #category : 'sessions' }
StLiveServer >> terminateSession: aSession [
	aSession job process isTerminated ifFalse: [ aSession job process terminate ].
	aSession state: #terminated
]

{ #category : 'sessions' }
StLiveServer >> sessionFor: anId [
	self checkRef: anId prefix: 'dbg'.
	^ sessions at: anId ifAbsent: [ (StLiveFailure code: 'unknown_session' message: 'Debug session ' , anId , ' is unknown (terminated or evicted)') signal ]
]

{ #category : 'transport' }
StLiveServer >> startPortFile: aPath [
	portFile := aPath.
	listener := Socket newTCP.
	listener listenOn: 0 backlogSize: 16 interface: #[ 127 0 0 1 ].
	running := true.
	StLiveTranscriptTee install.
	[ self acceptLoop ] forkAt: Processor userInterruptPriority named: 'stlive-acceptor'.
	aPath ifNotNil: [
		aPath asFileReference writeStreamDo: [ :s |
			s nextPutAll: (STON toJsonString: (OrderedDictionary new
				at: 'port' put: listener localPort;
				at: 'epoch' put: epoch;
				at: 'pharo' put: SystemVersion current version;
				yourself)) ] ].
	^ listener localPort
]

{ #category : 'transport' }
StLiveServer >> stop [
	running := false.
	listener ifNotNil: [ listener closeAndDestroy. listener := nil ]
]

{ #category : 'transport' }
StLiveServer >> acceptLoop [
	[ running ] whileTrue: [
		| client |
		client := listener waitForAcceptFor: 1.
		client ifNotNil: [
			[ self serve: client ] forkAt: Processor userInterruptPriority named: 'stlive-connection' ] ]
]

{ #category : 'transport' }
StLiveServer >> serve: aSocket [
	| stream line response |
	stream := SocketStream on: aSocket.
	stream binary.
	stream timeout: 30.
	[ line := stream upTo: 10.
	response := self handleLine: line utf8Decoded.
	stream nextPutAll: response utf8Encoded; nextPut: 10; flush ]
		on: Error
		do: [ :e | Stdio stdout nextPutAll: 'connection error: ' , e class name , ' ' , e messageText asString; lf; flush ].
	[ stream close ] on: Error do: [ :e | nil ].
	aSocket closeAndDestroy
]

{ #category : 'transport' }
StLiveServer >> jsonify: anObject [
	"Reduce to what JSON can carry: strings, numbers, booleans, nil, arrays, ordered maps."
	anObject isNil ifTrue: [ ^ nil ].
	(anObject == true or: [ anObject == false ]) ifTrue: [ ^ anObject ].
	anObject isSymbol ifTrue: [ ^ anObject asString ].
	anObject isString ifTrue: [ ^ anObject asString ].
	anObject isInteger ifTrue: [ ^ anObject ].
	anObject isFloat ifTrue: [ ^ (anObject isNaN or: [ anObject isInfinite ]) ifTrue: [ anObject printString ] ifFalse: [ anObject ] ].
	anObject isDictionary ifTrue: [
		| d |
		d := OrderedDictionary new.
		anObject keysAndValuesDo: [ :k :v | d at: k asString put: (self jsonify: v) ].
		^ d ].
	anObject isArray ifTrue: [ ^ anObject collect: [ :e | self jsonify: e ] ].
	anObject isCollection ifTrue: [ ^ anObject asArray collect: [ :e | self jsonify: e ] ].
	^ anObject printString
]

{ #category : 'transport' }
StLiveServer >> internalError: e [
	^ OrderedDictionary new
		at: 'ok' put: false;
		at: 'error' put: (OrderedDictionary new
			at: 'code' put: 'internal_error';
			at: 'message' put: e class name , ': ' , ([ e messageText asString ] on: Error do: [ :x | '?' ]);
			at: 'stack' put: ([ (e signalerContext stack first: 8) collect: [ :c | c printString ] ] on: Error do: [ :x | #() ]);
			yourself);
		yourself
]

{ #category : 'transport' }
StLiveServer >> handleLine: aString [
	| request response |
	response := [ [ [ request := STON fromString: aString. commands dispatch: request ]
		on: StLiveFailure do: [ :f |
			OrderedDictionary new at: 'ok' put: false; at: 'error' put: f asError; yourself ] ]
		on: Error do: [ :e | self internalError: e ] ]
		on: Warning do: [ :w | w resume ].
	response at: 'epoch' put: epoch.
	^ [ STON toJsonString: (self jsonify: response) ] on: Error do: [ :e |
		STON toJsonString: (OrderedDictionary new
			at: 'ok' put: false;
			at: 'epoch' put: epoch;
			at: 'error' put: (OrderedDictionary new at: 'code' put: 'serialization_error'; at: 'message' put: e messageText asString; yourself);
			yourself) ]
]
