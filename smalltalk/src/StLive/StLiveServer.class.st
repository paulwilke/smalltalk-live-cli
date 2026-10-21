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
		'portFile',
		'suppressed',
		'backgroundErrors',
		'previousErrorHandler',
		'acceptor',
		'connections',
		'session'
	],
	#classVars : [
		'Current',
		'InstalledHash',
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

{ #category : 'accessing' }
StLiveServer class >> reset [
	"Forget the singleton (its listener was stopped)."
	Current := nil
]

{ #category : 'accessing' }
StLiveServer class >> installedHash [
	"Hash of the embedded sources this image was loaded from (set by the CLI; used to detect stale servers)."
	^ InstalledHash
]

{ #category : 'accessing' }
StLiveServer class >> installedHash: aString [
	InstalledHash := aString
]

{ #category : 'accessing' }
StLiveServer class >> recordSystemPackagesFrom: names [
	"Used when attaching to an image that already contains application code."
	SystemPackages := names asSet
]

{ #category : 'starting' }
StLiveServer class >> startFromEnvironment [
	"STLIVE_PORTFILE: where to publish {port, epoch}. Without it the port is only printed. The result goes to the
	instance log (the image's stdout), so a start that does not publish a port can be explained from the log."
	| env path port |
	env := Smalltalk os environment.
	path := env at: 'STLIVE_PORTFILE' ifAbsent: [ nil ].
	port := self current startPortFile: path.
	"the server runs already: a failure to log this must not end the start"
	[ Stdio stdout
		nextPutAll: (path
			ifNil: [ 'stlive: listening on 127.0.0.1 port ' , port printString ]
			ifNotNil: [ :file | 'stlive: port file written ' , file , ' port ' , port printString ]);
		lf;
		flush ] on: Error do: [ :e | nil ].
	^ port
]

{ #category : 'session hooks' }
StLiveServer class >> registerSessionHandler [
	"SessionManager calls startUp: and shutDown: of this class at every start of the image and around every snapshot.
	Registering again replaces the earlier registration."
	SessionManager default registerUserClassNamed: self name
]

{ #category : 'session hooks' }
StLiveServer class >> startUp: isImageStarting [
	Current ifNotNil: [ :server | server startUp: isImageStarting ]
]

{ #category : 'session hooks' }
StLiveServer class >> shutDown: isImageQuitting [
	Current ifNotNil: [ :server | server shutDown: isImageQuitting ]
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
	connections := IdentitySet new.
	suppressed := IdentitySet new.
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

{ #category : 'background errors' }
StLiveServer >> backgroundErrors [
	"Unhandled errors of processes that are not stlive jobs (forked by application code), newest last."
	^ backgroundErrors ifNil: [ backgroundErrors := OrderedCollection new ]
]

{ #category : 'background errors' }
StLiveServer >> noteBackgroundError: anError in: aProcess [
	"Called by StLiveBackgroundErrorHandler on top of the failing stack: remember and log, never fail."
	| entry frames message |
	frames := [ (anError signalerContext stackOfSize: 10) collect: [ :c | c printString ] ] on: Error do: [ :e | #() ].
	message := [ anError messageText asString ] on: Error do: [ :e | '' ].
	message isEmpty ifTrue: [ message := [ anError description asString ] on: Error do: [ :e | anError class name ] ].
	entry := OrderedDictionary new
		at: 'time' put: DateAndTime now printString;
		at: 'process' put: ([ aProcess name asString trimBoth ] on: Error do: [ :e | '?' ]);
		at: 'type' put: anError class name;
		at: 'message' put: message;
		at: 'frames' put: frames asArray;
		yourself.
	self backgroundErrors addLast: entry.
	[ self backgroundErrors size > 50 ] whileTrue: [ self backgroundErrors removeFirst ].
	[ Stdio stdout
		nextPutAll: 'stlive: unhandled error in process ' , (entry at: 'process') , ' (terminated, image keeps running): ' , (entry at: 'type') , ': ' , (entry at: 'message'); lf.
	frames do: [ :f | Stdio stdout nextPutAll: '    ' , f; lf ].
	Stdio stdout flush ] on: Error do: [ :e | nil ]
]

{ #category : 'background errors' }
StLiveServer >> installBackgroundErrorHandler [
	"Headless images quit on an unhandled error in any process without its own handler (the default handler is
	the NonInteractiveUIManager). Replace it while the server runs. A GUI image keeps its debugger."
	| current |
	current := ErrorHandler defaultErrorHandler.
	(current isKindOf: StLiveBackgroundErrorHandler) ifTrue: [ ^ self ].
	(current isKindOf: MorphicUIManager) ifTrue: [ ^ self ].
	previousErrorHandler := current.
	ErrorHandler defaultErrorHandler: (StLiveBackgroundErrorHandler new server: self fallback: current; yourself)
]

{ #category : 'background errors' }
StLiveServer >> uninstallBackgroundErrorHandler [
	(ErrorHandler defaultErrorHandler isKindOf: StLiveBackgroundErrorHandler)
		ifTrue: [ ErrorHandler defaultErrorHandler: previousErrorHandler ]
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
	"Listens on 127.0.0.1, publishes the port (in aPath, when given) and starts the acceptor. The port file is written
	before the acceptor exists, so a failure here leaves no process that answers requests.
	When this server already listens (an attach to an image that runs it), the listener stays: the port already
	published keeps working, and the port file names it."
	| port |
	(running and: [ listener notNil and: [ listener isValid ] ])
		ifTrue: [
			port := listener localPort.
			self class registerSessionHandler.
			aPath ifNotNil: [ self writePortFile: aPath port: port ].
			self ensureAcceptor.
			^ port ].
	self stopAcceptor.
	self closeListener.
	portFile := aPath.
	session := Smalltalk session.
	listener := Socket newTCP.
	listener listenOn: 0 backlogSize: 16 interface: #[ 127 0 0 1 ].
	port := listener localPort.
	running := true.
	self class registerSessionHandler.
	StLiveTranscriptTee install.
	self subscribeToSystemChanges.
	[ self installBackgroundErrorHandler ] on: Error do: [ :e | nil ].
	aPath ifNotNil: [ self writePortFile: aPath port: port ].
	self startAcceptor.
	^ port
]

{ #category : 'transport' }
StLiveServer >> writePortFile: aPath port: aPort [
	aPath asFileReference writeStreamDo: [ :s |
		s nextPutAll: (STON toJsonString: (OrderedDictionary new
			at: 'port' put: aPort;
			at: 'epoch' put: epoch;
			at: 'pharo' put: SystemVersion current version;
			yourself)) ]
]

{ #category : 'transport' }
StLiveServer >> ensureAcceptor [
	"One live acceptor per image: an acceptor of an earlier version of this class (in an attached image) counts too."
	(Process allInstances anySatisfy: [ :p | p name = 'stlive-acceptor' and: [ p isTerminated not ] ])
		ifFalse: [ self startAcceptor ]
]

{ #category : 'transport' }
StLiveServer >> stop [
	running := false.
	self stopAcceptor.
	self uninstallBackgroundErrorHandler.
	self closeListener
]

{ #category : 'transport' }
StLiveServer >> startAcceptor [
	"The acceptor runs at userInterruptPriority, above jobs (userSchedulingPriority): status and debug answer while a
	job runs, and a job cannot keep the port from being served. At the job's priority a job that does not yield
	would starve the acceptor, so the priority stays."
	| process |
	process := [ self acceptLoop ] newProcess.
	process priority: Processor userInterruptPriority; name: 'stlive-acceptor'.
	acceptor := process.
	process resume
]

{ #category : 'transport' }
StLiveServer >> stopAcceptor [
	| process |
	process := acceptor.
	acceptor := nil.
	(process notNil and: [ process ~~ Processor activeProcess and: [ process isTerminated not ] ])
		ifTrue: [ process terminate ]
]

{ #category : 'transport' }
StLiveServer >> closeListener [
	| socket |
	socket := listener.
	listener := nil.
	socket ifNotNil: [ [ socket closeAndDestroy ] on: Error do: [ :e | nil ] ]
]

{ #category : 'transport' }
StLiveServer >> acceptLoop [
	"Body of the acceptor process. It ends as soon as it is no longer the acceptor, the server is stopped, its listener
	is closed or invalid, or the image was started again. A process saved with the image must never serve the
	listener of an earlier session: its sockets are gone, and waiting on them returns at once (a spin)."
	| mySession |
	mySession := session.
	[ acceptor == Processor activeProcess
		and: [ running
		and: [ listener notNil
		and: [ listener isValid
		and: [ Smalltalk session == mySession ] ] ] ] ] whileTrue: [
		| socket client |
		socket := listener.
		client := socket waitForAcceptFor: 1.
		client
			ifNil: [ (Delay forMilliseconds: 10) wait ]
			ifNotNil: [ self startConnectionFor: client ] ]
]

{ #category : 'transport' }
StLiveServer >> startConnectionFor: aSocket [
	| process |
	process := [ self serve: aSocket ] newProcess.
	process priority: Processor userInterruptPriority; name: 'stlive-connection'.
	self connectionProcesses add: process.
	process resume
]

{ #category : 'transport' }
StLiveServer >> connectionProcesses [
	"Processes serving connections of this session (nil in an image saved before this instance variable existed)."
	^ connections ifNil: [ connections := IdentitySet new ]
]

{ #category : 'transport' }
StLiveServer >> serve: aSocket [
	| stream line response |
	[ stream := SocketStream on: aSocket.
	stream binary.
	stream timeout: 30.
	[ line := stream upTo: 10.
	response := self handleLine: line utf8Decoded.
	stream nextPutAll: response utf8Encoded; nextPut: 10; flush ]
		on: Error
		do: [ :e |
			"A connection of an earlier session (a save that was resumed) has no socket any more: its answer is lost
			anyway, and reporting it would print into the stdout of whatever runs that image."
			aSocket isValid ifTrue: [
				Stdio stdout nextPutAll: 'connection error: ' , e class name , ' ' , e messageText asString; lf; flush ] ].
	[ stream close ] on: Error do: [ :e | nil ].
	aSocket closeAndDestroy ]
		ensure: [ self connectionProcesses remove: Processor activeProcess ifAbsent: [ nil ] ]
]

{ #category : 'session hooks' }
StLiveServer >> startUp: isImageStarting [
	"SessionManager hook, run when the image starts and after every snapshot. isImageStarting is true when the image
	was started from a saved file: the listener, the acceptor and the connections belong to the earlier session and
	are dropped (serve.st starts the server again). False: the session goes on after a save, its listener is still
	valid, so the acceptor starts again."
	[ isImageStarting
		ifTrue: [ self forgetEarlierSession ]
		ifFalse: [ (running and: [ listener notNil and: [ listener isValid ] ]) ifTrue: [ self startAcceptor ] ] ]
		on: Error
		do: [ :e | Stdio stdout nextPutAll: 'stlive: restarting the server after the snapshot failed: ' , e messageText asString; lf; flush ]
]

{ #category : 'session hooks' }
StLiveServer >> shutDown: isImageQuitting [
	"SessionManager hook, run before every snapshot (quitting or not). The acceptor is a process that must not be saved:
	a process saved in its loop runs first after the next start and spins on the old listener. Connections are left
	alone: the one that asked for the snapshot is waiting for its answer, the others are dropped at the next start.
	A quitting image closes its listener as well."
	[ self stopAcceptor.
	isImageQuitting ifTrue: [ self closeListener ] ]
		on: Error
		do: [ :e | Stdio stdout nextPutAll: 'stlive: stopping the server before the snapshot failed: ' , e messageText asString; lf; flush ]
]

{ #category : 'session hooks' }
StLiveServer >> forgetEarlierSession [
	"Drops what the earlier session left: the listener, the acceptor and the connections. Nothing is closed or
	terminated here. Terminating a process restored from the snapshot hangs the start (measured on Pharo 13.1); the
	dropped processes end by themselves: a socket of the earlier session is gone, so each one answers into an error
	(logged as a connection error) and finishes. An acceptor of the earlier session exits at its next check, because
	the session it belongs to is not the current one."
	running := false.
	acceptor := nil.
	listener := nil.
	self connectionProcesses removeAll
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

{ #category : 'recording' }
StLiveServer >> suppressRecordingDuring: aBlock [
	"Changes made by the tool itself (already logged) or by bulk loads are not recorded again.
	Per process: a job that stays suspended inside this block (a failing test, an interrupted load) must not
	silence changes made by other processes."
	| process added |
	process := Processor activeProcess.
	added := (suppressed includes: process) not.
	added ifTrue: [ suppressed add: process ].
	^ aBlock ensure: [ added ifTrue: [ suppressed remove: process ifAbsent: [ nil ] ] ]
]

{ #category : 'recording' }
StLiveServer >> isRecordingSuppressed [
	^ suppressed includes: Processor activeProcess
]

{ #category : 'recording' }
StLiveServer >> subscribeToSystemChanges [
	| announcer |
	announcer := SystemAnnouncer uniqueInstance.
	announcer unsubscribe: self.
	announcer weak
		when: MethodAdded send: #methodChanged: to: self;
		when: MethodModified send: #methodChanged: to: self;
		when: MethodRemoved send: #methodRemoved: to: self;
		when: ClassAdded send: #classChanged: to: self;
		when: ClassModifiedClassDefinition send: #classChanged: to: self;
		when: ClassCommented send: #classChanged: to: self
]

{ #category : 'recording' }
StLiveServer >> isOwnClass: aClass [
	^ (aClass instanceSide name beginsWith: 'StLive') or: [ aClass instanceSide name beginsWith: 'BaselineOfStLive' ]
]

{ #category : 'recording' }
StLiveServer >> methodChanged: announcement [
	"Eval-made (or GUI-made) method changes end up in the same change log as CLI compiles."
	| m cls old |
	self isRecordingSuppressed ifTrue: [ ^ self ].
	[ m := announcement method.
	cls := m methodClass.
	(cls isNil or: [ self isOwnClass: cls ]) ifTrue: [ ^ self ].
	m selector == #DoIt ifTrue: [ ^ self ].
	"a trait method copied into a using class is recorded for the trait only"
	((m respondsTo: #origin) and: [ m origin notNil and: [ m origin ~~ cls ] ]) ifTrue: [ ^ self ].
	old := (announcement respondsTo: #oldMethod) ifTrue: [ announcement oldMethod ] ifFalse: [ nil ].
	changes add: (OrderedDictionary new
		at: 'time' put: DateAndTime now printString;
		at: 'kind' put: 'method-compile';
		at: 'saved' put: false;
		at: 'origin' put: 'eval';
		at: 'package' put: (m package name);
		at: 'class_package' put: cls instanceSide package name;
		at: 'class_name' put: cls instanceSide name;
		at: 'class_kind' put: (commands kindOf: cls);
		at: 'side' put: (cls isMeta ifTrue: [ 'class' ] ifFalse: [ 'instance' ]);
		at: 'class' put: cls printString;
		at: 'selector' put: m selector;
		at: 'protocol' put: ([ m protocol name ] on: Error do: [ :e | 'as yet unclassified' ]);
		at: 'old_source' put: (old ifNotNil: [ :o | commands lf: o sourceCode ]);
		at: 'new_source' put: (commands canonicalSourceOf: m);
		yourself) ] on: Error do: [ :e | nil ]
]

{ #category : 'recording' }
StLiveServer >> methodRemoved: announcement [
	| m cls |
	self isRecordingSuppressed ifTrue: [ ^ self ].
	[ m := announcement method.
	cls := m methodClass.
	(cls isNil or: [ self isOwnClass: cls ]) ifTrue: [ ^ self ].
	((m respondsTo: #origin) and: [ m origin notNil and: [ m origin ~~ cls ] ]) ifTrue: [ ^ self ].
	changes add: (OrderedDictionary new
		at: 'time' put: DateAndTime now printString;
		at: 'kind' put: 'method-remove';
		at: 'saved' put: false;
		at: 'origin' put: 'eval';
		at: 'package' put: (m package name);
		at: 'class_package' put: cls instanceSide package name;
		at: 'class_name' put: cls instanceSide name;
		at: 'class_kind' put: (commands kindOf: cls);
		at: 'side' put: (cls isMeta ifTrue: [ 'class' ] ifFalse: [ 'instance' ]);
		at: 'class' put: cls printString;
		at: 'selector' put: m selector;
		at: 'old_source' put: (commands lf: m sourceCode);
		yourself) ] on: Error do: [ :e | nil ]
]

{ #category : 'recording' }
StLiveServer >> classChanged: announcement [
	| cls |
	self isRecordingSuppressed ifTrue: [ ^ self ].
	[ cls := (announcement respondsTo: #classAffected) ifTrue: [ announcement classAffected ] ifFalse: [ announcement classAdded ].
	(cls isNil or: [ cls isMeta or: [ self isOwnClass: cls ] ]) ifTrue: [ ^ self ].
	changes add: (commands classChangeEntry: cls origin: 'eval') ] on: Error do: [ :e | nil ]
]
