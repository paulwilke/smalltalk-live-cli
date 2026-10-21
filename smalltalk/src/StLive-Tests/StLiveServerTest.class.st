Class {
	#name : 'StLiveServerTest',
	#superclass : 'StLiveTestCase',
	#package : 'StLive-Tests'
}

{ #category : 'helpers' }
StLiveServerTest >> answerOf: aJsonString [
	"The response of handleLine: as a dictionary (the server answers JSON text)."
	^ STON fromString: (server handleLine: aJsonString)
]

{ #category : 'epoch' }
StLiveServerTest >> testEpochHasThreeLowercaseHexDigits [
	| epoch |
	epoch := StLiveServer newEpoch.
	self assert: epoch size equals: 3.
	epoch do: [ :c | self assert: ('0123456789abcdef' includes: c) ]
]

{ #category : 'jsonify' }
StLiveServerTest >> testJsonifyKeepsNilAndBooleans [
	self assert: (server jsonify: nil) isNil.
	self assert: (server jsonify: true) equals: true.
	self assert: (server jsonify: false) equals: false
]

{ #category : 'jsonify' }
StLiveServerTest >> testJsonifyTurnsSymbolsIntoStrings [
	self assert: (server jsonify: #foo) equals: 'foo'
]

{ #category : 'jsonify' }
StLiveServerTest >> testJsonifyKeepsNumbers [
	self assert: (server jsonify: 42) equals: 42.
	self assert: (server jsonify: 1.5) equals: 1.5
]

{ #category : 'jsonify' }
StLiveServerTest >> testJsonifyWritesNonFiniteFloatsAsText [
	self assert: (server jsonify: Float nan) equals: Float nan printString.
	self assert: (server jsonify: Float infinity) equals: Float infinity printString
]

{ #category : 'jsonify' }
StLiveServerTest >> testJsonifyDictionaryKeysBecomeStrings [
	| d |
	d := server jsonify: (Dictionary new at: #a put: #b; yourself).
	self assert: (d at: 'a') equals: 'b'
]

{ #category : 'jsonify' }
StLiveServerTest >> testJsonifyCollectionsBecomeArrays [
	self assert: (server jsonify: (OrderedCollection with: 1 with: #a with: nil)) equals: #(1 'a' nil)
]

{ #category : 'jsonify' }
StLiveServerTest >> testJsonifyOtherObjectsBecomeTheirPrintString [
	self assert: (server jsonify: Object) equals: 'Object'
]

{ #category : 'own classes' }
StLiveServerTest >> testStLiveAndBaselineClassesAreOwnClasses [
	self assert: (server isOwnClass: StLiveServer).
	self assert: (server isOwnClass: StLiveServer class).
	self assert: (server isOwnClass: (Smalltalk at: #BaselineOfStLive))
]

{ #category : 'own classes' }
StLiveServerTest >> testOtherClassesAreNotOwnClasses [
	self deny: (server isOwnClass: Object)
]

{ #category : 'suppression' }
StLiveServerTest >> testSuppressionIsScopedToTheBlock [
	| inside |
	inside := server suppressRecordingDuring: [ server isRecordingSuppressed ].
	self assert: inside.
	self deny: server isRecordingSuppressed
]

{ #category : 'suppression' }
StLiveServerTest >> testSuppressionIsClearedWhenTheBlockFails [
	[ server suppressRecordingDuring: [ Error signal: 'inside' ] ] on: Error do: [ :e | nil ].
	self deny: server isRecordingSuppressed
]

{ #category : 'suppression' }
StLiveServerTest >> testNestedSuppressionKeepsTheOuterOne [
	| inner |
	inner := server suppressRecordingDuring: [
		server suppressRecordingDuring: [ nil ].
		server isRecordingSuppressed ].
	self assert: inner
]

{ #category : 'dispatch' }
StLiveServerTest >> testPingIsDispatched [
	| response |
	response := commands dispatch: (commands m: { 'cmd' -> 'ping' }).
	self assert: (response at: 'ok').
	self assert: ((response at: 'result') at: 'pong')
]

{ #category : 'dispatch' }
StLiveServerTest >> testDottedCommandNamesReachTheirSelector [
	self assert: (commands dispatch: (commands m: { 'cmd' -> 'changes.list' })) keys asArray equals: #('ok' 'result')
]

{ #category : 'dispatch' }
StLiveServerTest >> testUnknownCommandFails [
	| failure |
	failure := self failureOf: [ commands dispatch: (commands m: { 'cmd' -> 'no.such' }) ].
	self assert: failure code equals: 'unknown_command'
]

{ #category : 'dispatch' }
StLiveServerTest >> testRequestWithoutCommandFails [
	| failure |
	failure := self failureOf: [ commands dispatch: (commands m: {}) ].
	self assert: failure code equals: 'bad_request'
]

{ #category : 'lines' }
StLiveServerTest >> testHandleLineAnswersJsonWithTheEpoch [
	| response |
	response := self answerOf: '{"cmd":"ping"}'.
	self assert: (response at: 'ok').
	self assert: (response at: 'epoch') equals: server epoch
]

{ #category : 'lines' }
StLiveServerTest >> testHandleLineReportsAnUnknownCommandAsAnError [
	| response |
	response := self answerOf: '{"cmd":"no.such"}'.
	self deny: (response at: 'ok').
	self assert: ((response at: 'error') at: 'code') equals: 'unknown_command'
]

{ #category : 'lines' }
StLiveServerTest >> testHandleLineReportsInvalidJsonAsInternalError [
	| response |
	response := self answerOf: 'not json at all'.
	self deny: (response at: 'ok').
	self assert: ((response at: 'error') at: 'code') equals: 'internal_error'
]

{ #category : 'lines' }
StLiveServerTest >> testInternalErrorNamesTheExceptionClass [
	| response |
	response := server internalError: (Error new messageText: 'boom').
	self assert: ((response at: 'error') at: 'message') equals: 'Error: boom'
]

{ #category : 'session hooks' }
StLiveServerTest >> testSessionHandlerIsRegistered [
	StLiveServer registerSessionHandler.
	self assert: (SessionManager default hasRegistered: 'StLiveServer')
]

{ #category : 'session hooks' }
StLiveServerTest >> testShutDownStopsTheAcceptorProcess [
	| process |
	process := [ Semaphore new wait ] newProcess.
	process resume.
	server instVarNamed: 'acceptor' put: process.
	server shutDown: false.
	self assert: process isTerminated.
	self assert: (server instVarNamed: 'acceptor') isNil
]

{ #category : 'session hooks' }
StLiveServerTest >> testShutDownBeforeASaveKeepsTheListener [
	| socket |
	socket := Socket newTCP.
	server instVarNamed: 'listener' put: socket.
	server shutDown: false.
	self assert: (server instVarNamed: 'listener') identicalTo: socket.
	socket closeAndDestroy
]

{ #category : 'session hooks' }
StLiveServerTest >> testShutDownBeforeQuittingClosesTheListener [
	server instVarNamed: 'listener' put: Socket newTCP.
	server shutDown: true.
	self assert: (server instVarNamed: 'listener') isNil
]

{ #category : 'session hooks' }
StLiveServerTest >> testStartUpOfASavedImageForgetsTheEarlierSession [
	| process socket |
	process := [ Semaphore new wait ] newProcess.
	process resume.
	socket := Socket newTCP.
	server instVarNamed: 'running' put: true.
	server instVarNamed: 'listener' put: socket.
	server instVarNamed: 'acceptor' put: process.
	server startUp: true.
	self deny: (server instVarNamed: 'running').
	self assert: (server instVarNamed: 'listener') isNil.
	self assert: (server instVarNamed: 'acceptor') isNil.
	socket closeAndDestroy.
	process terminate
]

{ #category : 'session hooks' }
StLiveServerTest >> testStartUpOfASavedImageDropsItsConnections [
	| process |
	process := [ Semaphore new wait ] newProcess.
	process resume.
	server connectionProcesses add: process.
	server startUp: true.
	self assert: server connectionProcesses isEmpty.
	process terminate
]

{ #category : 'session hooks' }
StLiveServerTest >> testStartUpWithAStoppedServerStartsNoAcceptor [
	server instVarNamed: 'running' put: false.
	server startUp: false.
	self assert: (server instVarNamed: 'acceptor') isNil
]

{ #category : 'session hooks' }
StLiveServerTest >> testStartUpAfterASaveRestartsTheAcceptorOnTheSameListener [
	| socket |
	socket := Socket newTCP listenOn: 0 backlogSize: 1 interface: #[ 127 0 0 1 ]; yourself.
	server instVarNamed: 'running' put: true.
	server instVarNamed: 'session' put: Smalltalk session.
	server instVarNamed: 'listener' put: socket.
	server startUp: false.
	self assert: (server instVarNamed: 'acceptor') notNil.
	self deny: (server instVarNamed: 'acceptor') isTerminated.
	server shutDown: true
]

{ #category : 'session hooks' }
StLiveServerTest >> testAcceptLoopEndsWhenTheListenerIsInvalid [
	| socket |
	socket := Socket newTCP.
	socket closeAndDestroy.
	server instVarNamed: 'running' put: true.
	server instVarNamed: 'session' put: Smalltalk session.
	server instVarNamed: 'listener' put: socket.
	server instVarNamed: 'acceptor' put: Processor activeProcess.
	server acceptLoop.
	server instVarNamed: 'acceptor' put: nil
]

{ #category : 'session hooks' }
StLiveServerTest >> testAcceptLoopEndsInAnotherSession [
	| socket |
	socket := Socket newTCP listenOn: 0 backlogSize: 1 interface: #[ 127 0 0 1 ]; yourself.
	server instVarNamed: 'running' put: true.
	server instVarNamed: 'session' put: Object new.
	server instVarNamed: 'listener' put: socket.
	server instVarNamed: 'acceptor' put: Processor activeProcess.
	server acceptLoop.
	server instVarNamed: 'acceptor' put: nil.
	socket closeAndDestroy
]
