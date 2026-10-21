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
