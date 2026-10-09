"
Stand-in for the global Transcript in images whose Transcript has no replaceable stream (GUI images:
StThreadSafeTranscript). Text written by a job's worker is captured per job; everything else
(and every message not about writing text) is forwarded to the real Transcript, so the Transcript window keeps working.
"
Class {
	#name : 'StLiveTranscriptProxy',
	#superclass : 'ProtoObject',
	#instVars : [
		'original'
	],
	#package : 'StLive'
}

{ #category : 'accessing' }
StLiveTranscriptProxy >> original [
	^ original
]

{ #category : 'accessing' }
StLiveTranscriptProxy >> original: aTranscript [
	original := aTranscript
]

{ #category : 'private' }
StLiveTranscriptProxy >> currentJob [
	| handler |
	handler := Processor activeProcess errorHandler.
	^ (handler isKindOf: StLiveErrorHandler) ifTrue: [ handler job ] ifFalse: [ nil ]
]

{ #category : 'private' }
StLiveTranscriptProxy >> capture: aString else: aBlock [
	^ self currentJob
		ifNil: [ aBlock value ]
		ifNotNil: [ :job | job captureOutput: aString. self ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> show: anObject [
	^ self capture: anObject displayString else: [ original show: anObject ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> nextPutAll: aString [
	^ self capture: aString asString else: [ original nextPutAll: aString ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> nextPut: aCharacter [
	^ self capture: (String with: aCharacter) else: [ original nextPut: aCharacter ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> cr [
	^ self capture: String lf else: [ original cr ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> lf [
	^ self capture: String lf else: [ original lf ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> newLine [
	^ self capture: String lf else: [ original newLine ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> tab [
	^ self capture: String tab else: [ original tab ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> space [
	^ self capture: ' ' else: [ original space ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> << anObject [
	^ self capture: anObject displayString else: [ original << anObject ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> print: anObject [
	^ self capture: anObject printString else: [ original print: anObject ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> display: anObject [
	^ self capture: anObject displayString else: [ original display: anObject ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> crShow: anObject [
	^ self capture: String lf , anObject displayString else: [ original crShow: anObject ]
]

{ #category : 'writing' }
StLiveTranscriptProxy >> showCr: anObject [
	^ self capture: anObject displayString , String lf else: [ original showCr: anObject ]
]

{ #category : 'testing' }
StLiveTranscriptProxy >> isStLiveProxy [
	^ true
]

{ #category : 'forwarding' }
StLiveTranscriptProxy >> doesNotUnderstand: aMessage [
	^ aMessage sendTo: original
]

{ #category : 'testing' }
StLiveTranscriptProxy >> yourself [
	"ProtoObject does not define it; without this a cascade ending in yourself would answer the forwarded original."
	^ self
]

{ #category : 'testing' }
StLiveTranscriptProxy >> class [
	^ StLiveTranscriptProxy
]
