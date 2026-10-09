"
Replaces the stream of the Transcript: text written by a job's worker process is captured per job
(and reported separately in the response); everything else goes to the original stream.
"
Class {
	#name : 'StLiveTranscriptTee',
	#superclass : 'Object',
	#instVars : [
		'original'
	],
	#package : 'StLive'
}

{ #category : 'installing' }
StLiveTranscriptTee class >> install [
	"Never let a problem here prevent the server from starting: output capture is a convenience."
	[ self installUnsafely ]
		on: Error
		do: [ :e | Stdio stdout nextPutAll: 'stlive: transcript capture disabled (' , e class name , ': ' , e messageText asString , ')'; lf; flush ]
]

{ #category : 'installing' }
StLiveTranscriptTee class >> installUnsafely [
	| current |
	(Smalltalk globals at: #Transcript) class == StLiveTranscriptProxy ifTrue: [ ^ self ].
	"Headless images: the Transcript writes to a replaceable stream."
	(Transcript class allInstVarNames includes: #stream)
		ifTrue: [
			current := Transcript instVarNamed: 'stream'.
			current isNil ifTrue: [ current := Transcript stream ].
			(current isKindOf: self) ifTrue: [ ^ self ].
			^ Transcript instVarNamed: 'stream' put: (self new original: current; yourself) ].
	"GUI images (StThreadSafeTranscript, announcer based): stand in for the global."
	Smalltalk globals at: #Transcript put: (StLiveTranscriptProxy new original: Transcript; yourself)
]

{ #category : 'accessing' }
StLiveTranscriptTee >> original: aStream [
	original := aStream
]

{ #category : 'private' }
StLiveTranscriptTee >> currentJob [
	| handler |
	handler := Processor activeProcess errorHandler.
	^ (handler isKindOf: StLiveErrorHandler) ifTrue: [ handler job ] ifFalse: [ nil ]
]

{ #category : 'writing' }
StLiveTranscriptTee >> nextPutAll: aString [
	^ self currentJob
		ifNil: [ original nextPutAll: aString ]
		ifNotNil: [ :job | job captureOutput: aString ]
]

{ #category : 'writing' }
StLiveTranscriptTee >> nextPut: aCharacter [
	^ self nextPutAll: (String with: aCharacter)
]

{ #category : 'writing' }
StLiveTranscriptTee >> flush [
	^ original flush
]

{ #category : 'writing' }
StLiveTranscriptTee >> close [
	^ original close
]

{ #category : 'writing' }
StLiveTranscriptTee >> doesNotUnderstand: aMessage [
	^ aMessage sendTo: original
]
