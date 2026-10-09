"
One unit of evaluated work (an eval, a single test, ...) running in its own worker process.
Errors in the worker are not unwound: StLiveErrorHandler parks the process inside the
failing execution and registers a StLiveSession, so the live stack stays inspectable.
"
Class {
	#name : 'StLiveJob',
	#superclass : 'Object',
	#instVars : [
		'id',
		'server',
		'block',
		'process',
		'outcome',
		'value',
		'events',
		'origin',
		'session',
		'startedAt',
		'output',
		'warnings'
	],
	#package : 'StLive'
}

{ #category : 'instance creation' }
StLiveJob class >> server: aServer id: anId block: aBlock origin: aDictionary [
	^ self new setServer: aServer id: anId block: aBlock origin: aDictionary
]

{ #category : 'accessing' }
StLiveJob >> id [
	^ id
]

{ #category : 'accessing' }
StLiveJob >> server [
	^ server
]

{ #category : 'accessing' }
StLiveJob >> process [
	^ process
]

{ #category : 'accessing' }
StLiveJob >> outcome [
	^ outcome
]

{ #category : 'accessing' }
StLiveJob >> value [
	^ value
]

{ #category : 'accessing' }
StLiveJob >> origin [
	^ origin
]

{ #category : 'accessing' }
StLiveJob >> session [
	^ session
]

{ #category : 'initialization' }
StLiveJob >> setServer: aServer id: anId block: aBlock origin: aDictionary [
	server := aServer.
	id := anId.
	block := aBlock.
	origin := aDictionary.
	events := Semaphore new.
	output := WriteStream on: String new.
	warnings := Bag new.
	outcome := #new.
	startedAt := DateAndTime now
]

{ #category : 'running' }
StLiveJob >> start [
	outcome := #running.
	process := [ self runBody: block ] newProcess.
	process
		priority: Processor userSchedulingPriority;
		name: 'stlive-' , id;
		errorHandler: (StLiveErrorHandler new job: self; yourself).
	process resume
]

{ #category : 'running' }
StLiveJob >> runBody: aBlock [
	"The stack of a session ends at this frame (see StLiveSession>>isBoundary:)."
	| result |
	result := [ aBlock value ] on: Deprecation do: [ :d |
		self addWarning: d messageText.
		d resume ].
	self finishedWith: result
]

{ #category : 'running' }
StLiveJob >> finishedWith: anObject [
	value := anObject.
	outcome := #done.
	events signal
]

{ #category : 'running' }
StLiveJob >> erroredWith: aSession [
	session := aSession.
	outcome := #error.
	events signal
]

{ #category : 'running' }
StLiveJob >> markRunning [
	outcome := #running.
	value := nil
]

{ #category : 'running' }
StLiveJob >> waitEvent: milliseconds [
	"Answer #done, #error, #terminated, or #timeout (the process is then still running)."
	(events waitTimeoutMSecs: milliseconds) ifTrue: [
		(outcome == #running and: [ process isTerminated ]) ifTrue: [ outcome := #terminated. ^ outcome ].
		^ #timeout ].
	^ outcome
]

{ #category : 'running' }
StLiveJob >> interrupt [
	"Suspend a still running worker and capture where it is, like the user interrupt (Cmd-.) does."
	process suspend.
	outcome == #running ifFalse: [ ^ outcome ].
	process isTerminated ifTrue: [ ^ outcome := #terminated ].
	session := server
		newSessionFor: self
		exception: nil
		context: process suspendedContext
		kind: #interrupted.
	session markSuspended.
	outcome := #error.
	^ outcome
]

{ #category : 'running' }
StLiveJob >> terminate [
	process isTerminated ifFalse: [ process terminate ].
	outcome := #terminated
]

{ #category : 'output' }
StLiveJob >> captureOutput: aString [
	output size < 100000 ifTrue: [ output nextPutAll: aString asString ]
]

{ #category : 'output' }
StLiveJob >> addWarning: aString [
	warnings add: aString asString
]

{ #category : 'output' }
StLiveJob >> takeReport [
	"Answer { transcript, warnings } collected since the last call, or nil when there was nothing."
	| text report |
	text := output contents.
	(text isEmpty and: [ warnings isEmpty ]) ifTrue: [ ^ nil ].
	report := OrderedDictionary new.
	text isEmpty ifFalse: [
		report at: 'transcript' put: (text size > 4000 ifTrue: [ (text first: 4000) ] ifFalse: [ text ]).
		text size > 4000 ifTrue: [ report at: 'transcript_truncated' put: true ] ].
	warnings isEmpty ifFalse: [
		report at: 'deprecations' put: (warnings asSet asArray collect: [ :w |
			OrderedDictionary new at: 'message' put: w; at: 'count' put: (warnings occurrencesOf: w); yourself ]) ].
	output := WriteStream on: String new.
	warnings := Bag new.
	^ report
]
