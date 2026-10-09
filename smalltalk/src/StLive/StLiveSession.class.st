"
A debug session: a suspended execution kept alive in the image, addressable by id.
"
Class {
	#name : 'StLiveSession',
	#superclass : 'Object',
	#instVars : [
		'id',
		'job',
		'debugSession',
		'exception',
		'kind',
		'state',
		'topContext',
		'createdAt',
		'resumeValue',
		'typeName',
		'messageText',
		'continuedFrom',
		'exceptionRef'
	],
	#package : 'StLive'
}

{ #category : 'instance creation' }
StLiveSession class >> id: anId job: aJob exception: anException context: aContext kind: aKind [
	^ self new setId: anId job: aJob exception: anException context: aContext kind: aKind
]

{ #category : 'constants' }
StLiveSession class >> maxFrames [
	"Stacks are walked at most this deep (endless recursion has millions of frames)."
	^ 100000
]

{ #category : 'initialization' }
StLiveSession >> setId: anId job: aJob exception: anException context: aContext kind: aKind [
	id := anId.
	job := aJob.
	exception := anException.
	kind := aKind.
	state := #new.
	createdAt := DateAndTime now.
	topContext := self skipHelperFrames: aContext.
	typeName := anException ifNil: [ 'Interrupted' ] ifNotNil: [ anException class name ].
	messageText := self describeException.
	debugSession := job process
		newDebugSessionNamed: messageText
		startedAt: topContext.
	exception ifNotNil: [ debugSession exception: exception ]
]

{ #category : 'initialization' }
StLiveSession >> describeException [
	exception ifNil: [ ^ 'Execution interrupted (timeout)' ].
	^ [ | text |
		text := exception messageText.
		(text isNil or: [ text isEmpty ]) ifTrue: [ text := exception description ].
		text asString ] on: Error do: [ :e | exception class name , ' (message unavailable)' ]
]

{ #category : 'initialization' }
StLiveSession >> skipHelperFrames: aContext [
	"Like the Pharo debugger: do not stop on error:, doesNotUnderstand: and friends."
	| ctx |
	ctx := aContext.
	[ ctx notNil and: [ ctx sender notNil and: [ self isHelper: ctx ] ] ]
		whileTrue: [ ctx := ctx sender ].
	^ ctx
]

{ #category : 'testing' }
StLiveSession >> isHelper: aContext [
	^ [ aContext method hasPragmaNamed: #debuggerCompleteToSender ] on: Error do: [ :e | false ]
]

{ #category : 'testing' }
StLiveSession >> isBoundary: aContext [
	| r |
	r := aContext receiver.
	(r == job and: [ aContext selector == #runBody: ]) ifTrue: [ ^ true ].
	(r isKindOf: StLiveCommands) ifTrue: [ ^ true ].
	"the doit invocation frame right above our own tooling frames"
	^ aContext selector == #valueWithReceiver:arguments: and: [ aContext sender notNil and: [ aContext sender receiver isKindOf: StLiveCommands ] ]
]

{ #category : 'accessing' }
StLiveSession >> id [
	^ id
]

{ #category : 'accessing' }
StLiveSession >> job [
	^ job
]

{ #category : 'accessing' }
StLiveSession >> debugSession [
	^ debugSession
]

{ #category : 'accessing' }
StLiveSession >> exception [
	^ exception
]

{ #category : 'accessing' }
StLiveSession >> kind [
	^ kind
]

{ #category : 'accessing' }
StLiveSession >> state [
	^ state
]

{ #category : 'accessing' }
StLiveSession >> state: aSymbol [
	state := aSymbol
]

{ #category : 'accessing' }
StLiveSession >> typeName [
	^ typeName
]

{ #category : 'accessing' }
StLiveSession >> messageText [
	^ messageText
]

{ #category : 'accessing' }
StLiveSession >> createdAt [
	^ createdAt
]

{ #category : 'accessing' }
StLiveSession >> topContext [
	^ topContext
]

{ #category : 'accessing' }
StLiveSession >> continuedFrom [
	^ continuedFrom
]

{ #category : 'accessing' }
StLiveSession >> continuedFrom: aSessionId [
	continuedFrom := aSessionId
]

{ #category : 'accessing' }
StLiveSession >> process [
	^ job process
]

{ #category : 'accessing' }
StLiveSession >> origin [
	^ job origin
]

{ #category : 'state' }
StLiveSession >> markSuspended [
	state := #suspended
]

{ #category : 'state' }
StLiveSession >> isSuspended [
	^ state == #suspended
]

{ #category : 'state' }
StLiveSession >> awaitParked [
	"The worker signals the waiting command just before it suspends itself; give it a moment."
	| tries |
	tries := 0.
	[ self process isSuspended or: [ self process isTerminated ] ] whileFalse: [
		tries := tries + 1.
		tries > 200 ifTrue: [ ^ false ].
		(Delay forMilliseconds: 5) wait ].
	^ self process isSuspended
]

{ #category : 'state' }
StLiveSession >> resumeValue: anObject [
	resumeValue := anObject
]

{ #category : 'state' }
StLiveSession >> takeResumeValue [
	| v |
	v := resumeValue.
	resumeValue := nil.
	^ v
]

{ #category : 'frames' }
StLiveSession >> contextAt: anIndex [
	"Answer the context of frame anIndex (0 is the top frame) or nil."
	| ctx i |
	ctx := topContext.
	i := 0.
	[ ctx notNil and: [ (self isBoundary: ctx) not ] ] whileTrue: [
		i = anIndex ifTrue: [ ^ ctx ].
		i := i + 1.
		ctx := ctx sender ].
	^ nil
]

{ #category : 'frames' }
StLiveSession >> contextsFrom: offset limit: limit [
	"Answer an association: array of contexts (the requested page) -> total frame count."
	| ctx i page |
	ctx := topContext.
	i := 0.
	page := OrderedCollection new.
	[ ctx notNil and: [ i < self class maxFrames and: [ (self isBoundary: ctx) not ] ] ] whileTrue: [
		(i >= offset and: [ page size < limit ]) ifTrue: [ page add: ctx ].
		i := i + 1.
		ctx := ctx sender ].
	^ page asArray -> i
]

{ #category : 'frames' }
StLiveSession >> frameCount [
	^ (self contextsFrom: 0 limit: 0) value
]

{ #category : 'frames' }
StLiveSession >> indexOfContext: aContext [
	| ctx i |
	ctx := topContext.
	i := 0.
	[ ctx notNil and: [ (self isBoundary: ctx) not ] ] whileTrue: [
		ctx == aContext ifTrue: [ ^ i ].
		i := i + 1.
		ctx := ctx sender ].
	^ nil
]
